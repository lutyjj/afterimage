//! Drawing a screen document: its data sources fetched, its templates resolved,
//! its images fetched, and the result handed to the renderer.
//!
//! Templates are `MiniJinja` (Jinja syntax). Text without template syntax is itself;
//! a template that is exactly one `{{ expression }}` keeps the expression's value,
//! so a chart's series can be a list; any other template renders to text. Data
//! source requests read `secrets`; nothing else does, so a secret never reaches
//! the screen. Each problem names where in the document it arose, as a JSON
//! Pointer, so the editor can point at it.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use minijinja::value::{Serde, Value as Template};
use minijinja::{Environment, State, UndefinedBehavior, context};
use afterimage_render_typst::{Image, Renderer};
use afterimage_trmnl::catalog::{Model, Palette};
use serde_json::{Map, Value, json};
use tokio::sync::Semaphore;

use crate::secrets::Secrets;

/// How long a data source or image may take.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest data source answer read.
const SOURCE_BYTES: usize = 2 * 1024 * 1024;
/// Largest image read.
const IMAGE_BYTES: usize = 4 * 1024 * 1024;
/// Template steps one render may take, which bounds loops over large data.
const FUEL: u64 = 200_000;

/// The device a screen is drawn for, as templates read it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Device {
    pub name: String,
    /// Layout pixels.
    pub width: f64,
    pub height: f64,
}

/// What went wrong while drawing, and where in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub message: String,
    /// A JSON Pointer into the document, such as `/content/column/children/0`.
    pub path: Option<String>,
}

/// What each data source answered: its JSON, or why it gave none.
pub type SourceResults = BTreeMap<String, Result<Value, String>>;

/// The outcome of drawing.
pub struct Drawn {
    /// Absent when the renderer could not draw the screen.
    pub image: Option<Image>,
    pub sources: SourceResults,
    /// Empty when everything resolved.
    pub problems: Vec<Problem>,
}

/// Owns what every drawing shares: the template environment, the HTTP client,
/// the renderer and a bound on renders at once.
pub struct Drawing {
    templates: Environment<'static>,
    http: reqwest::Client,
    renderer: Arc<Renderer>,
    renders: Semaphore,
    secrets: Secrets,
    time_zone: jiff::tz::TimeZone,
}

impl Drawing {
    /// # Panics
    ///
    /// When the HTTP client cannot be built, which only a broken TLS setup causes.
    #[must_use]
    pub fn new(secrets: Secrets, time_zone: jiff::tz::TimeZone) -> Self {
        let mut templates = Environment::new();
        minijinja_contrib::add_to_environment(&mut templates);
        templates.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        templates.set_undefined_behavior(UndefinedBehavior::Chainable);
        templates.set_fuel(Some(FUEL));
        // The time of the render, not of the call, so a screen is drawn at one
        // instant; as minijinja-contrib's `now`, a Unix timestamp for its filters.
        templates.add_function("now", |state: &State| {
            state.lookup("__render_time").unwrap_or_default()
        });
        let http = reqwest::Client::builder()
            .timeout(FETCH_TIMEOUT)
            .user_agent(concat!("afterimage/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("the HTTP client builds");
        Self {
            templates,
            http,
            renderer: Arc::new(Renderer::new()),
            renders: Semaphore::new(
                std::thread::available_parallelism().map_or(2, std::num::NonZero::get),
            ),
            secrets,
            time_zone,
        }
    }

    #[must_use]
    pub fn time_zone(&self) -> &jiff::tz::TimeZone {
        &self.time_zone
    }

    /// Draws a document, in the model's serde form, at `now` (Unix seconds).
    pub async fn draw(
        &self,
        document: &Value,
        device: &Device,
        model: &Model,
        palette: &Palette,
        now: i64,
    ) -> Drawn {
        let time_zone = self.time_zone.iana_name().unwrap_or("UTC").to_owned();
        #[allow(clippy::cast_precision_loss)]
        let base = context! {
            device => Template::from(Serde(device)),
            __render_time => now as f64,
            TIMEZONE => time_zone,
        };
        let mut resolver = Resolver {
            templates: &self.templates,
            problems: Vec::new(),
            images: Vec::new(),
        };
        let sources = self.fetch_sources(document, &base, &mut resolver).await;
        let data: BTreeMap<&str, Template> = sources
            .iter()
            .map(|(name, answer)| {
                let value = answer
                    .as_ref()
                    .map_or(Template::UNDEFINED, |value| Template::from(Serde(value)));
                (name.as_str(), value)
            })
            .collect();
        let scope = context! { ..Template::from(Serde(&data)), ..base };
        let screen = resolver.document(document, &scope);
        let images = self.fetch_images(&mut resolver).await;
        let Resolver { problems, .. } = resolver;
        let image = match self.render(screen, images, model, palette).await {
            Ok(image) => Some(image),
            Err(message) => {
                let mut problems = problems;
                problems.push(Problem {
                    message,
                    path: None,
                });
                return Drawn {
                    image: None,
                    sources,
                    problems,
                };
            }
        };
        Drawn {
            image,
            sources,
            problems,
        }
    }

    /// Draws a document whose values are final, such as the server's own screens.
    ///
    /// # Errors
    ///
    /// When the renderer cannot draw it.
    pub async fn draw_resolved(
        &self,
        resolved: Value,
        model: &Model,
        palette: &Palette,
    ) -> Result<Image, String> {
        self.render(resolved, BTreeMap::new(), model, palette).await
    }

    async fn render(
        &self,
        resolved: Value,
        images: BTreeMap<String, Vec<u8>>,
        model: &Model,
        palette: &Palette,
    ) -> Result<Image, String> {
        let _place = self
            .renders
            .acquire()
            .await
            .map_err(|error| error.to_string())?;
        let renderer = Arc::clone(&self.renderer);
        let (model, palette) = (model.clone(), palette.clone());
        tokio::task::spawn_blocking(move || renderer.render(&resolved, &images, &model, &palette))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }

    async fn fetch_sources(
        &self,
        document: &Value,
        base: &Template,
        resolver: &mut Resolver<'_>,
    ) -> SourceResults {
        let Some(sources) = document.get("sources").and_then(Value::as_object) else {
            return SourceResults::new();
        };
        let scope = context! { secrets => self.secrets.values(), ..base.clone() };
        let mut requests = tokio::task::JoinSet::new();
        for (name, source) in sources {
            let path = format!("/sources/{}", pointer_escape(name));
            let request = resolver
                .request(source, &scope, &path)
                .map(|request| self.http.request(request.0, request.1));
            let name = name.clone();
            match request {
                Ok(builder) => {
                    let builder = match source.get("headers").and_then(Value::as_object) {
                        Some(headers) => {
                            headers.iter().fold(builder, |builder, (header, value)| {
                                let value = resolver.text(
                                    value,
                                    &scope,
                                    &format!("{path}/headers/{}", pointer_escape(header)),
                                );
                                builder.header(header.as_str(), value)
                            })
                        }
                        None => builder,
                    };
                    let builder =
                        match source.get("body").and_then(Value::as_str) {
                            Some(body) => builder.header("Content-Type", "application/json").body(
                                resolver.text(
                                    &Value::String(body.to_owned()),
                                    &scope,
                                    &format!("{path}/body"),
                                ),
                            ),
                            None => builder,
                        };
                    requests.spawn(async move { (name, fetch_json(builder).await) });
                }
                Err(message) => {
                    requests.spawn(async move { (name, Err(message)) });
                }
            }
        }
        let mut results = SourceResults::new();
        while let Some(joined) = requests.join_next().await {
            if let Ok((name, answer)) = joined {
                if let Err(message) = &answer {
                    resolver.problems.push(Problem {
                        message: format!("Data source `{name}`: {message}"),
                        path: Some(format!("/sources/{}", pointer_escape(&name))),
                    });
                }
                results.insert(name, answer);
            }
        }
        results
    }

    async fn fetch_images(&self, resolver: &mut Resolver<'_>) -> BTreeMap<String, Vec<u8>> {
        let mut images = BTreeMap::new();
        for (key, url, path) in std::mem::take(&mut resolver.images) {
            match self.fetch_image(&url).await {
                Ok(bytes) => {
                    images.insert(key, bytes);
                }
                Err(message) => resolver.problems.push(Problem {
                    message: format!("Image {url}: {message}"),
                    path: Some(path),
                }),
            }
        }
        images
    }

    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, String> {
        if url.starts_with("data:") {
            let data = data_url::DataUrl::process(url).map_err(|error| format!("{error:?}"))?;
            let (bytes, _) = data.decode_to_vec().map_err(|error| format!("{error:?}"))?;
            return Ok(bytes);
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("an image URL starts with http://, https:// or data:".to_owned());
        }
        read_limited(self.http.get(url), IMAGE_BYTES).await
    }
}

/// Sends the request and reads at most `limit` bytes of a successful answer.
async fn read_limited(request: reqwest::RequestBuilder, limit: usize) -> Result<Vec<u8>, String> {
    let mut response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("answered HTTP {status}"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > limit {
            return Err(format!("answered more than {limit} bytes"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn fetch_json(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let bytes = read_limited(request.header("Accept", "application/json"), SOURCE_BYTES).await?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("answered something other than JSON: {error}"))
}

/// A key for JSON Pointer: `~` and `/` escaped.
fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// The inner expression of a template that is exactly one `{{ expression }}`.
fn single_expression(text: &str) -> Option<&str> {
    let inner = text.trim().strip_prefix("{{")?.strip_suffix("}}")?;
    let inner = inner.trim_start_matches('-').trim_end_matches('-');
    (!inner.contains("{{") && !inner.contains("}}") && !inner.contains("{%")).then_some(inner)
}

fn has_template(text: &str) -> bool {
    text.contains("{{") || text.contains("{%") || text.contains("{#")
}

/// Resolves a document's templates, collecting problems and the images to fetch.
struct Resolver<'a> {
    templates: &'a Environment<'static>,
    problems: Vec<Problem>,
    /// Key, URL and document path of each image.
    images: Vec<(String, String, String)>,
}

impl Resolver<'_> {
    fn problem(&mut self, path: &str, message: impl Into<String>) {
        self.problems.push(Problem {
            message: message.into(),
            path: Some(path.to_owned()),
        });
    }

    /// A template's value: its expression's value, or its text.
    fn value(&mut self, text: &str, scope: &Template, path: &str) -> Value {
        if !has_template(text) {
            return Value::String(text.to_owned());
        }
        let result = match single_expression(text) {
            Some(expression) => self
                .templates
                .compile_expression(expression)
                .and_then(|compiled| compiled.eval(scope))
                .map(|value| serde_json::to_value(value).unwrap_or(Value::Null)),
            None => self.templates.render_str(text, scope).map(Value::String),
        };
        result.unwrap_or_else(|error| {
            self.problem(path, error.to_string());
            Value::Null
        })
    }

    /// A template as text.
    fn text(&mut self, template: &Value, scope: &Template, path: &str) -> String {
        match self.value(template.as_str().unwrap_or_default(), scope, path) {
            Value::String(text) => text,
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    /// A data source's method and URL.
    fn request(
        &mut self,
        source: &Value,
        scope: &Template,
        path: &str,
    ) -> Result<(reqwest::Method, String), String> {
        let url = self.text(&source["url"], scope, &format!("{path}/url"));
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(format!(
                "the URL `{url}` does not start with http:// or https://"
            ));
        }
        let method = if source.get("method").and_then(Value::as_str) == Some("POST") {
            reqwest::Method::POST
        } else {
            reqwest::Method::GET
        };
        Ok((method, url))
    }

    fn document(&mut self, document: &Value, scope: &Template) -> Value {
        let mut resolved = Map::new();
        if let Some(bar) = document.get("titleBar") {
            let mut bar = self.fields(bar, scope, "/titleBar");
            self.take_image(&mut bar, "icon", "/titleBar/icon");
            resolved.insert("titleBar".to_owned(), bar);
        }
        let content = self.component(&document["content"], scope, "/content");
        resolved.insert("content".to_owned(), content);
        Value::Object(resolved)
    }

    /// Replaces the URL at `member` with the key of the image it names.
    fn take_image(&mut self, object: &mut Value, member: &str, path: &str) {
        let Some(url) = object
            .get(member)
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return;
        };
        if url.is_empty() {
            object[member] = Value::Null;
            return;
        }
        let key = format!("image-{}", self.images.len());
        self.images.push((key.clone(), url, path.to_owned()));
        object[member] = Value::String(key);
    }

    /// Every string member as a template; components and lists of them recurse.
    fn fields(&mut self, value: &Value, scope: &Template, path: &str) -> Value {
        let Some(object) = value.as_object() else {
            return value.clone();
        };
        let mut resolved = Map::new();
        for (name, member) in object {
            let here = format!("{path}/{}", pointer_escape(name));
            let member = match (name.as_str(), member) {
                ("children", Value::Array(children)) => Value::Array(
                    children
                        .iter()
                        .enumerate()
                        .map(|(index, child)| {
                            self.component(child, scope, &format!("{here}/{index}"))
                        })
                        .collect(),
                ),
                ("content", child) => self.component(child, scope, &here),
                ("cells" | "series", Value::Array(items)) => Value::Array(
                    items
                        .iter()
                        .enumerate()
                        .map(|(index, item)| self.fields(item, scope, &format!("{here}/{index}")))
                        .collect(),
                ),
                (_, Value::String(text)) => self.value(text, scope, &here),
                (_, other) => other.clone(),
            };
            resolved.insert(name.clone(), member);
        }
        Value::Object(resolved)
    }

    fn component(&mut self, node: &Value, scope: &Template, path: &str) -> Value {
        let Some((kind, value)) = node.as_object().and_then(|object| object.iter().next()) else {
            self.problem(path, "a component is an object with one member");
            return json!({ "column": { "children": [] } });
        };
        let here = format!("{path}/{kind}");
        match kind.as_str() {
            "each" => self.each(value, scope, &here),
            "image" => {
                let mut image = self.fields(value, scope, &here);
                self.take_image(&mut image, "url", &format!("{here}/url"));
                json!({ "image": image })
            }
            _ => {
                let mut resolved = Map::new();
                resolved.insert(kind.clone(), self.fields(value, scope, &here));
                Value::Object(resolved)
            }
        }
    }

    /// An `each` becomes a column or row of its content once per element, and a
    /// line saying how many were left out.
    fn each(&mut self, each: &Value, scope: &Template, path: &str) -> Value {
        let items = match self.value(
            each["items"].as_str().unwrap_or_default(),
            scope,
            &format!("{path}/items"),
        ) {
            Value::Array(items) => items,
            Value::Null => Vec::new(),
            other => {
                self.problem(
                    &format!("{path}/items"),
                    format!("`items` is {other}, not a list"),
                );
                Vec::new()
            }
        };
        let limit = each
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(items.len(), |limit| {
                usize::try_from(limit).unwrap_or(usize::MAX)
            });
        let mut children: Vec<Value> = items
            .iter()
            .take(limit)
            .map(|item| {
                let scope = context! { item => Template::from(Serde(item)), ..scope.clone() };
                self.component(&each["content"], &scope, &format!("{path}/content"))
            })
            .collect();
        let hidden = items.len().saturating_sub(limit);
        if hidden > 0 {
            let scope = context! { hidden => hidden, ..scope.clone() };
            let more = self.text(&each["more"], &scope, &format!("{path}/more"));
            children.push(json!({ "label": { "text": more, "size": "small", "style": "plain" } }));
        }
        let direction = each
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("column")
            .to_owned();
        let mut stack = Map::new();
        stack.insert(
            direction,
            json!({
                "children": children,
                "gap": each.get("gap").cloned().unwrap_or(json!("medium")),
                "align": "start",
                "justify": "start",
                "grow": false,
            }),
        );
        Value::Object(stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(document: &Value, scope: &Template) -> (Value, Vec<Problem>) {
        let mut templates = Environment::new();
        minijinja_contrib::add_to_environment(&mut templates);
        templates.set_undefined_behavior(UndefinedBehavior::Chainable);
        let mut resolver = Resolver {
            templates: &templates,
            problems: Vec::new(),
            images: Vec::new(),
        };
        let screen = resolver.document(document, scope);
        (screen, resolver.problems)
    }

    #[test]
    fn plain_text_stays_text_and_a_single_expression_keeps_its_type() {
        let scope = context! { weather => Template::from(Serde(json!({ "temperature": 7.44, "forecast": [{ "t": 1 }, { "t": 3 }] }))) };
        let document = json!({ "content": { "column": { "children": [
            { "value": { "text": "{{ weather.temperature | round(1) }}°" } },
            { "lineChart": { "series": [{ "values": "{{ weather.forecast | map(attribute='t') | list }}" }] } },
            { "label": { "text": "Plain {not a template}" } }
        ] } } });
        let (resolved, problems) = resolve(&document, &scope);
        assert_eq!(problems, []);
        let children = &resolved["content"]["column"]["children"];
        assert_eq!(children[0]["value"]["text"], "7.4°");
        assert_eq!(
            children[1]["lineChart"]["series"][0]["values"],
            json!([1, 3])
        );
        assert_eq!(children[2]["label"]["text"], "Plain {not a template}");
    }

    #[test]
    fn each_repeats_its_content_and_counts_what_it_leaves_out() {
        let scope =
            context! { calendar => Template::from(Serde(json!({ "events": ["A", "B", "C"] }))) };
        let document = json!({ "content": { "each": {
            "items": "{{ calendar.events }}",
            "limit": 2,
            "more": "and {{ hidden }} more",
            "content": { "title": { "text": "{{ item }}" } }
        } } });
        let (resolved, problems) = resolve(&document, &scope);
        assert_eq!(problems, []);
        let children = &resolved["content"]["column"]["children"];
        assert_eq!(children[0]["title"]["text"], "A");
        assert_eq!(children[1]["title"]["text"], "B");
        assert_eq!(children[2]["label"]["text"], "and 1 more");
        assert_eq!(children.as_array().unwrap().len(), 3);
    }

    #[test]
    fn a_broken_template_is_a_problem_at_its_path() {
        let document = json!({ "content": { "row": { "children": [
            { "value": { "text": "{{ 1 + }}" } }
        ] } } });
        let (_, problems) = resolve(&document, &context! {});
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].path.as_deref(),
            Some("/content/row/children/0/value/text")
        );
    }

    #[test]
    fn images_become_keys_for_their_bytes() {
        let document = json!({
            "titleBar": { "title": "T", "icon": "https://icons.example.test/a.svg" },
            "content": { "image": { "url": "data:image/png;base64,AA==" } }
        });
        let mut templates = Environment::new();
        let mut resolver = Resolver {
            templates: &mut templates,
            problems: Vec::new(),
            images: Vec::new(),
        };
        let screen = resolver.document(&document, &context! {});
        assert_eq!(screen["titleBar"]["icon"], "image-0");
        assert_eq!(screen["content"]["image"]["url"], "image-1");
        assert_eq!(resolver.images[1].1, "data:image/png;base64,AA==");
    }

    #[test]
    fn single_expressions_are_recognised_only_whole() {
        assert_eq!(single_expression(" {{ a.b }} "), Some(" a.b "));
        assert_eq!(single_expression("{{ a }} and {{ b }}"), None);
        assert_eq!(single_expression("{{ a }}°"), None);
    }
}
