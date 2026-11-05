//! The parts of an `OpenAPI` document that make up the TRMNL device contract, so that
//! TRMNL's published document and the one generated from `model/` compare member by
//! member with one extraction.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The operations TRMNL firmware calls, as `(method, path)`.
pub const DEVICE_OPERATIONS: [(&str, &str); 4] = [
    ("get", "/api/setup"),
    ("get", "/api/display"),
    ("get", "/api/display/current"),
    ("post", "/api/log"),
];

/// Where TRMNL publishes its API document.
pub const TRMNL_DOCUMENT: &str = "https://trmnl.com/api-docs/openapi.json";

/// The device contract taken from TRMNL's document, with where and when it came from.
#[derive(Debug, Serialize, Deserialize)]
pub struct Excerpt {
    pub source: String,
    /// The UTC date the document was fetched, as `YYYY-MM-DD`.
    pub retrieved: String,
    #[serde(flatten)]
    pub contract: Contract,
}

/// Operations keyed by `METHOD path`, such as `GET /api/display`.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub operations: BTreeMap<String, Operation>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    /// The document's `operationId`.
    pub name: String,
    /// The status code of the success response.
    pub success: String,
    /// Request headers by name as the document writes it.
    pub headers: BTreeMap<String, Member>,
    /// Top-level fields of the JSON request body.
    pub body: BTreeMap<String, Member>,
    /// Top-level fields of the JSON success response.
    pub fields: BTreeMap<String, Member>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub required: bool,
    /// The JSON Schema `type`, such as `string` or `integer`.
    #[serde(rename = "type")]
    pub kind: String,
}

/// Extracts the given operations from an `OpenAPI` 3 document.
///
/// # Errors
///
/// When an operation is missing or a member has no resolvable type.
pub fn extract(doc: &Value, operations: &[(&str, &str)]) -> Result<Contract, String> {
    let mut extracted = BTreeMap::new();
    for &(method, path) in operations {
        let item = doc
            .pointer(&format!("/paths/{}", escape(path)))
            .ok_or_else(|| format!("{path}: no such path"))?;
        let operation = item
            .get(method)
            .ok_or_else(|| format!("{path}: no {method} operation"))?;
        extracted.insert(
            format!("{} {path}", method.to_uppercase()),
            Operation {
                name: text(operation, "operationId").unwrap_or_default(),
                success: success_code(operation)
                    .ok_or_else(|| format!("{path}: no 2xx response"))?,
                headers: headers(doc, item, operation)?,
                body: body(doc, operation)?,
                fields: fields(doc, operation)?,
            },
        );
    }
    Ok(Contract {
        operations: extracted,
    })
}

/// Every operation in the document, for a document that holds only device operations.
///
/// # Errors
///
/// As [`extract`].
pub fn extract_all(doc: &Value) -> Result<Contract, String> {
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .ok_or("the document has no paths")?;
    let operations: Vec<(&str, &str)> = paths
        .iter()
        .flat_map(|(path, item)| {
            ["get", "put", "post", "delete", "patch"]
                .into_iter()
                .filter(|method| item.get(*method).is_some())
                .map(move |method| (method, path.as_str()))
        })
        .collect();
    extract(doc, &operations)
}

fn headers(
    doc: &Value,
    item: &Value,
    operation: &Value,
) -> Result<BTreeMap<String, Member>, String> {
    let shared = item.get("parameters").and_then(Value::as_array);
    let own = operation.get("parameters").and_then(Value::as_array);
    let mut headers = BTreeMap::new();
    for parameter in shared.into_iter().chain(own).flatten() {
        let parameter = resolve(doc, parameter)?;
        if text(parameter, "in").as_deref() != Some("header") {
            continue;
        }
        let name = text(parameter, "name").ok_or("a header parameter has no name")?;
        let schema = parameter
            .get("schema")
            .ok_or_else(|| format!("header {name}: no schema"))?;
        headers.insert(
            name.clone(),
            Member {
                required: parameter
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                kind: kind(doc, schema).ok_or_else(|| format!("header {name}: no type"))?,
            },
        );
    }
    Ok(headers)
}

fn body(doc: &Value, operation: &Value) -> Result<BTreeMap<String, Member>, String> {
    let Some(request) = operation.get("requestBody") else {
        return Ok(BTreeMap::new());
    };
    match json_schema(doc, resolve(doc, request)?)? {
        Some(schema) => members(doc, schema),
        None => Ok(BTreeMap::new()),
    }
}

fn fields(doc: &Value, operation: &Value) -> Result<BTreeMap<String, Member>, String> {
    let code = success_code(operation).unwrap_or_default();
    let Some(response) = operation.pointer(&format!("/responses/{code}")) else {
        return Ok(BTreeMap::new());
    };
    match json_schema(doc, resolve(doc, response)?)? {
        Some(schema) => members(doc, schema),
        None => Ok(BTreeMap::new()),
    }
}

/// The resolved schema of a request body or response's `application/json` content.
fn json_schema<'a>(doc: &'a Value, holder: &'a Value) -> Result<Option<&'a Value>, String> {
    holder
        .pointer("/content/application~1json/schema")
        .map(|schema| resolve(doc, schema))
        .transpose()
}

fn members(doc: &Value, schema: &Value) -> Result<BTreeMap<String, Member>, String> {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(BTreeMap::new());
    };
    properties
        .iter()
        .map(|(name, property)| {
            let member = Member {
                required: required.contains(&name.as_str()),
                kind: kind(doc, property).ok_or_else(|| format!("field {name}: no type"))?,
            };
            Ok((name.clone(), member))
        })
        .collect()
}

fn kind(doc: &Value, schema: &Value) -> Option<String> {
    text(resolve(doc, schema).ok()?, "type")
}

fn success_code(operation: &Value) -> Option<String> {
    operation
        .get("responses")?
        .as_object()?
        .keys()
        .filter(|code| code.starts_with('2'))
        .min()
        .cloned()
}

/// Follows a local `$ref` such as `#/components/schemas/Name`.
fn resolve<'a>(doc: &'a Value, value: &'a Value) -> Result<&'a Value, String> {
    match value.get("$ref").and_then(Value::as_str) {
        Some(reference) => {
            let pointer = reference
                .strip_prefix('#')
                .ok_or_else(|| format!("{reference}: not a local reference"))?;
            doc.pointer(pointer)
                .ok_or_else(|| format!("{reference}: not found"))
        }
        None => Ok(value),
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Escapes a path for use as one JSON Pointer token.
fn escape(path: &str) -> String {
    path.replace('~', "~0").replace('/', "~1")
}
