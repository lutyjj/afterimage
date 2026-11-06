//! The tools, from the `OpenAPI` document the model emits.
//!
//! An operation is a tool when the model tags it [`TAG`]. The tool's name is the
//! operation's ID, its description is the operation's documentation, and its
//! input is the operation's input structure: path labels and body members share
//! one namespace in Smithy, so they share one here. Nothing is restated, so an
//! operation joins the tools by its tag alone.

use std::collections::BTreeSet;

use bytes::Bytes;
use http::{HeaderValue, Method, Request, header};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Map, Value};

/// The tag `model/` puts on an operation to offer it as a tool.
pub const TAG: &str = "mcp";

const SCHEMAS: &str = "#/components/schemas/";
const METHODS: [&str; 5] = ["get", "put", "post", "delete", "patch"];

/// What a path segment escapes: everything but RFC 3986's unreserved characters.
const ESCAPED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("the OpenAPI document is not JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The document holds something a tool cannot carry. Refused at startup,
    /// since a tool that drops an input is worse than no endpoint.
    #[error("{operation} cannot be a tool: {reason}")]
    Unsupported { operation: String, reason: String },
    #[error("the OpenAPI document has no GET operation {0} without inputs")]
    NoCheck(String),
}

/// Every tool, in name order, and what the API says of itself.
#[derive(Debug)]
pub struct Catalog {
    /// The API's description, which tells a client how its operations fit
    /// together.
    pub instructions: Option<String>,
    /// The path of the operation that answers whether a credential is good.
    pub(crate) check: String,
    tools: Vec<Tool>,
}

#[derive(Debug)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Map<String, Value>,
    pub method: Method,
    path: String,
    path_parameters: Vec<String>,
    body_members: Option<BTreeSet<String>>,
    /// Members of the JSON answer whose bytes the protocol writes as base64.
    pub(crate) binary_members: Vec<String>,
}

impl Catalog {
    /// The tools of `document`, and `check`, the ID of a GET operation without
    /// inputs that answers 2xx exactly for a credential the API accepts.
    ///
    /// # Errors
    ///
    /// When the document is not JSON, holds no `check`, or tags an operation a
    /// tool cannot carry.
    pub fn from_openapi(document: &str, check: &str) -> Result<Self, CatalogError> {
        let document: Value = serde_json::from_str(document)?;
        let schemas = document
            .pointer("/components/schemas")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut tools = Vec::new();
        let mut check_path = None;
        for (path, item) in document
            .get("paths")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            for method in METHODS {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                if method == "get"
                    && operation.get("operationId").and_then(Value::as_str) == Some(check)
                    && operation.get("parameters").is_none()
                {
                    check_path = Some(path.clone());
                }
                let tagged = operation
                    .get("tags")
                    .and_then(Value::as_array)
                    .is_some_and(|tags| tags.iter().any(|tag| tag == TAG));
                if tagged {
                    tools.push(tool(path, method, operation, &schemas)?);
                }
            }
        }
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            instructions: document
                .pointer("/info/description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            check: check_path.ok_or_else(|| CatalogError::NoCheck(check.to_owned()))?,
            tools,
        })
    }

    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name == name)
    }
}

impl Tool {
    /// The API request these arguments make, sent with the caller's own
    /// `Authorization`.
    ///
    /// # Errors
    ///
    /// When an argument is not one of the tool's inputs, or a path label is
    /// missing or not a scalar; the message says what to change.
    ///
    /// # Panics
    ///
    /// Never: a JSON object always serialises.
    pub fn request(
        &self,
        arguments: &Map<String, Value>,
        authorization: HeaderValue,
    ) -> Result<Request<Bytes>, String> {
        let known = |name: &String| {
            self.path_parameters.contains(name)
                || self
                    .body_members
                    .as_ref()
                    .is_some_and(|members| members.contains(name))
        };
        if let Some(unknown) = arguments.keys().find(|name| !known(name)) {
            return Err(format!("`{unknown}` is not an input of {}", self.name));
        }

        let mut target = self.path.clone();
        for name in &self.path_parameters {
            let value = match arguments.get(name) {
                Some(Value::String(text)) => text.clone(),
                Some(value @ (Value::Number(_) | Value::Bool(_))) => value.to_string(),
                Some(_) => return Err(format!("`{name}` must be a string or a number")),
                None => return Err(format!("`{name}` is required")),
            };
            let encoded = utf8_percent_encode(&value, ESCAPED).to_string();
            target = target.replace(&format!("{{{name}}}"), &encoded);
        }

        let mut request = Request::builder()
            .method(self.method.clone())
            .uri(target)
            .header(header::AUTHORIZATION, authorization);
        let body = match &self.body_members {
            Some(members) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                let body: Map<String, Value> = arguments
                    .iter()
                    .filter(|(name, _)| members.contains(*name))
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                Bytes::from(serde_json::to_vec(&body).expect("a JSON object serialises"))
            }
            None => Bytes::new(),
        };
        request
            .body(body)
            .map_err(|error| format!("the request cannot be made: {error}"))
    }
}

fn tool(
    path: &str,
    method: &str,
    operation: &Value,
    schemas: &Map<String, Value>,
) -> Result<Tool, CatalogError> {
    let name = operation
        .get("operationId")
        .and_then(Value::as_str)
        .unwrap_or(path)
        .to_owned();
    let refuse = |reason: String| CatalogError::Unsupported {
        operation: name.clone(),
        reason,
    };

    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut path_parameters = Vec::new();
    for parameter in operation
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let member = parameter
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if parameter.get("in").and_then(Value::as_str) != Some("path") {
            return Err(refuse(format!("`{member}` is not a path label")));
        }
        path_parameters.push(member.to_owned());
        required.push(Value::from(member));
        let mut schema = parameter
            .get("schema")
            .cloned()
            .unwrap_or(Value::Object(Map::new()));
        if let (Some(text), Some(object)) = (parameter.get("description"), schema.as_object_mut()) {
            object.insert("description".to_owned(), text.clone());
        }
        properties.insert(member.to_owned(), schema);
    }

    let body_members = match operation.pointer("/requestBody/content/application~1json/schema") {
        Some(schema) => {
            let body = resolve(schema, schemas)
                .ok_or_else(|| refuse("its body schema is missing".to_owned()))?;
            let members = body
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            required.extend(
                body.get("required")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            let names = members.keys().cloned().collect();
            for (member, schema) in members {
                if properties.insert(member.clone(), schema).is_some() {
                    return Err(refuse(format!(
                        "`{member}` is both a path label and a body member"
                    )));
                }
            }
            Some(names)
        }
        None if operation.get("requestBody").is_some() => {
            return Err(refuse("its body is not JSON".to_owned()));
        }
        None => None,
    };

    let binary_members = binary_members(operation, schemas);
    let input_schema = input_schema(properties, required, schemas).map_err(refuse)?;

    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| refuse("it has no documentation to describe it".to_owned()))?
        .to_owned();

    Ok(Tool {
        name,
        description,
        input_schema,
        method: method
            .to_uppercase()
            .parse()
            .expect("one of the five methods"),
        path: path.to_owned(),
        path_parameters,
        body_members,
        binary_members,
    })
}

/// Members of the operation's JSON answer whose bytes the protocol writes as
/// base64.
fn binary_members(operation: &Value, schemas: &Map<String, Value>) -> Vec<String> {
    operation
        .get("responses")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(status, _)| status.starts_with('2'))
        .filter_map(|(_, response)| response.pointer("/content/application~1json/schema"))
        .filter_map(|schema| resolve(schema, schemas))
        .filter_map(|schema| schema.get("properties").and_then(Value::as_object))
        .flatten()
        .filter(|(_, schema)| schema.get("format").and_then(Value::as_str) == Some("byte"))
        .map(|(member, _)| member.clone())
        .collect()
}

/// A tool's input schema, standing alone: what it refers to comes with it.
fn input_schema(
    properties: Map<String, Value>,
    required: Vec<Value>,
    schemas: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut properties = Value::Object(properties);
    let mut reached = BTreeSet::new();
    point_at_definitions(&mut properties, &mut reached);
    let mut definitions = Map::new();
    while let Some(next) = reached
        .iter()
        .find(|name| !definitions.contains_key(*name))
        .cloned()
    {
        let mut schema = schemas
            .get(&next)
            .cloned()
            .ok_or_else(|| format!("`{next}` is not a schema of the document"))?;
        point_at_definitions(&mut schema, &mut reached);
        definitions.insert(next, schema);
    }
    let mut input = Map::new();
    input.insert("type".to_owned(), "object".into());
    input.insert("properties".to_owned(), properties);
    input.insert("required".to_owned(), required.into());
    input.insert("additionalProperties".to_owned(), false.into());
    if !definitions.is_empty() {
        input.insert("$defs".to_owned(), Value::Object(definitions));
    }
    Ok(input)
}

/// A schema, or the one it refers to.
fn resolve<'a>(schema: &'a Value, schemas: &'a Map<String, Value>) -> Option<&'a Value> {
    match schema.get("$ref").and_then(Value::as_str) {
        Some(reference) => schemas.get(reference.strip_prefix(SCHEMAS)?),
        None => Some(schema),
    }
}

/// Points every reference at the tool's own `$defs`, recording what it reaches.
fn point_at_definitions(value: &mut Value, reached: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(name) = reference.strip_prefix(SCHEMAS)
            {
                reached.insert(name.to_owned());
                *reference = format!("#/$defs/{name}");
            }
            object
                .values_mut()
                .for_each(|child| point_at_definitions(child, reached));
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|child| point_at_definitions(child, reached)),
        _ => {}
    }
}
