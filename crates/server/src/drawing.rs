//! Drawing a screen document: its data sources fetched, its templates resolved,
//! its images fetched, and the result handed to the renderer.
//!
//! Templates are `MiniJinja` (Jinja syntax). Text without template syntax is itself;
//! a template that is exactly one `{{ expression }}` keeps the expression's value,
//! so a chart's series can be a list; any other template renders to text. Data
//! source requests read `secrets`; nothing else does, and every answer and message
//! has their values redacted, so a secret never reaches the screen or the editor.
//! Each problem names where in the document it arose, as a JSON Pointer, so the
//! editor can point at it.
//!
//! A drawing is bounded, since a device's check-in waits on it: its fetches end
//! within [`FETCH_TIME`], its templates run for at most [`RESOLVE_TIME`], and it
//! gives up when it cannot start its templates or its render within
//! [`PLACE_TIME`]. A document expands to at most [`MAX_COMPONENTS`] components and
//! [`MAX_IMAGES`] images.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use afterimage_render_typst::{Image, Renderer};
use afterimage_trmnl::catalog::{Catalog, Model, Palette};
use minijinja::value::{Kwargs, Serde, Value as Template};
use minijinja::{Environment, Error, ErrorKind, State, UndefinedBehavior, context};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value, json};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::outbound::Outbound;
use crate::secrets::Secrets;
use crate::time_zone::TimeZone;

/// How long a data source or image may take.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a drawing's fetches may take together. Firmware 1.8.18 waits 15
/// seconds for the check-in's answer (`HTTP_DEFAULT_TIMEOUT_MS`,
/// `http_retry_request.h`), and templates and the render come after the fetches.
const FETCH_TIME: Duration = Duration::from_secs(11);
/// How long after it starts a drawing may still wait for a CPU place to begin
/// its templates or its render; past it, it gives up rather than answer late.
const PLACE_TIME: Duration = Duration::from_secs(13);
/// How long templates may run over one document.
const RESOLVE_TIME: Duration = Duration::from_secs(2);
/// Largest data source answer read.
const SOURCE_BYTES: usize = 2 * 1024 * 1024;
/// Largest image read.
const IMAGE_BYTES: usize = 4 * 1024 * 1024;
/// Template steps one template may take, which bounds loops over large data.
const FUEL: u64 = 200_000;
/// Components one document may expand to, `each` included.
pub const MAX_COMPONENTS: usize = 2_000;
/// Distinct images one document may show.
pub const MAX_IMAGES: usize = 16;
/// Elements an `each` without a `limit` draws, the model's largest `limit`.
const EACH_LIMIT: usize = 100;
/// Problems reported for one drawing; the rest are counted.
const MAX_PROBLEMS: usize = 32;

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
/// the renderer and a bound on CPU-bound work at once.
pub struct Drawing {
    templates: Arc<Environment<'static>>,
    http: Outbound,
    renderer: Arc<Renderer>,
    /// One place per CPU, held while templates resolve and while Typst renders.
    places: Arc<Semaphore>,
    secrets: Secrets,
    time_zone: TimeZone,
}

/// The environment every template renders in: `MiniJinja` with `minijinja-contrib`
/// and its Python methods, Home Assistant's `multiply`, and `now()`.
fn templates() -> Environment<'static> {
    let mut templates = Environment::new();
    minijinja_contrib::add_to_environment(&mut templates);
    templates.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
    templates.set_undefined_behavior(UndefinedBehavior::Chainable);
    templates.set_fuel(Some(FUEL));
    templates.add_filter("multiply", multiply);
    // The time of the render, not of the call, so a screen is drawn at one
    // instant; as minijinja-contrib's `now`, a Unix timestamp for its filters.
    templates.add_function("now", |state: &State| {
        state.lookup("__render_time").unwrap_or_default()
    });
    templates
}

/// Home Assistant's `multiply(value, amount, default)`: a number, or text that
/// reads as one, times `by`, so `map('multiply', 0.01)` turns amounts in minor
/// units into a series. A value that is no number gives `default`, positional
/// or named, and without one is a problem.
// MiniJinja hands keyword arguments to a filter by value only.
#[allow(clippy::needless_pass_by_value)]
fn multiply(
    value: &Template,
    by: f64,
    default: Option<Template>,
    named: Kwargs,
) -> Result<Template, Error> {
    let default = match default {
        Some(default) => Some(default),
        None => named.get::<Option<Template>>("default")?,
    };
    named.assert_all_used()?;
    let number = f64::try_from(value.clone())
        .ok()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()));
    match (number, default) {
        (Some(number), _) => Ok(Template::from(number * by)),
        (None, Some(default)) => Ok(default),
        (None, None) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("multiply needs a number, not {value}"),
        )),
    }
}

impl Drawing {
    #[must_use]
    pub fn new(secrets: Secrets, time_zone: TimeZone) -> Self {
        let templates = templates();
        Self {
            templates: Arc::new(templates),
            http: Outbound::new(FETCH_TIMEOUT),
            renderer: Arc::new(Renderer::new()),
            places: Arc::new(Semaphore::new(
                std::thread::available_parallelism().map_or(2, std::num::NonZero::get),
            )),
            secrets,
            time_zone,
        }
    }

    #[must_use]
    pub fn time_zone(&self) -> &TimeZone {
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
        let started = Instant::now();
        let deadline = started + FETCH_TIME;
        #[allow(clippy::cast_precision_loss)]
        let base = context! {
            device => Template::from(Serde(device)),
            __render_time => now as f64,
            TIMEZONE => self.time_zone.name(),
        };
        let mut resolver = Resolver::new(Arc::clone(&self.templates));
        let sources = self
            .fetch_sources(document, &base, &mut resolver, deadline)
            .await;
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
        let places_by = started + PLACE_TIME;
        let (screen, mut resolver) = match self
            .resolve(resolver, document.clone(), scope, places_by)
            .await
        {
            Ok(done) => done,
            Err(message) => {
                return Drawn {
                    image: None,
                    sources,
                    problems: vec![Problem {
                        message,
                        path: None,
                    }],
                };
            }
        };
        let images = self.fetch_images(&mut resolver, deadline).await;
        let mut problems = resolver.finish();
        let image = match self.render(screen, images, model, palette, places_by).await {
            Ok(image) => Some(image),
            Err(message) => {
                problems.push(Problem {
                    message,
                    path: None,
                });
                None
            }
        };
        for problem in &mut problems {
            problem.message = self.secrets.redact(&problem.message);
        }
        Drawn {
            image,
            sources,
            problems,
        }
    }

    /// The latest moment a drawing starting now may begin its templates or its
    /// render; a caller drawing several things for one check-in shares it.
    #[must_use]
    pub fn places_deadline() -> Instant {
        Instant::now() + PLACE_TIME
    }

    /// Draws a document whose values are final, such as the server's own screens,
    /// giving up when no place is free by `by`.
    ///
    /// # Errors
    ///
    /// When the renderer cannot draw it, or no place is free in time.
    pub async fn draw_resolved(
        &self,
        resolved: Value,
        model: &Model,
        palette: &Palette,
        by: Instant,
    ) -> Result<Image, String> {
        self.render(resolved, BTreeMap::new(), model, palette, by)
            .await
    }

    /// One of the places, or a failure once `by` passes without one.
    async fn place(&self, by: Instant) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        tokio::time::timeout_at(by, Arc::clone(&self.places).acquire_owned())
            .await
            .map_err(|_| "the server is busy drawing other screens; try again".to_owned())?
            .map_err(|error| error.to_string())
    }

    /// Resolves the document's templates. They run over data of any size, so they
    /// run off the async workers, holding one of the places.
    async fn resolve(
        &self,
        mut resolver: Resolver,
        document: Value,
        scope: Template,
        by: Instant,
    ) -> Result<(Value, Resolver), String> {
        let place = self.place(by).await?;
        tokio::task::spawn_blocking(move || {
            let _place = place;
            let screen = resolver.document(&document, &scope);
            (screen, resolver)
        })
        .await
        .map_err(|error| error.to_string())
    }

    async fn render(
        &self,
        resolved: Value,
        images: BTreeMap<String, Vec<u8>>,
        model: &Model,
        palette: &Palette,
        by: Instant,
    ) -> Result<Image, String> {
        // The place moves into the blocking task, so it stays held until the render
        // ends even when the caller stops waiting.
        let place = self.place(by).await?;
        let renderer = Arc::clone(&self.renderer);
        let (model, palette) = (model.clone(), palette.clone());
        tokio::task::spawn_blocking(move || {
            let _place = place;
            renderer.render(&resolved, &images, &model, &palette)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
    }

    async fn fetch_sources(
        &self,
        document: &Value,
        base: &Template,
        resolver: &mut Resolver,
        deadline: Instant,
    ) -> SourceResults {
        let Some(sources) = document.get("sources").and_then(Value::as_object) else {
            return SourceResults::new();
        };
        let scope = context! { secrets => self.secrets.values(), ..base.clone() };
        let mut requests = tokio::task::JoinSet::new();
        for (name, source) in sources {
            let path = format!("/sources/{}", pointer_escape(name));
            let request = resolver.request(self.http.client(), source, &scope, &path);
            let name = name.clone();
            match request {
                Ok(builder) => {
                    let http = self.http.clone();
                    requests.spawn(async move {
                        let answer = tokio::time::timeout_at(deadline, fetch_json(&http, builder))
                            .await
                            .unwrap_or_else(|_| Err(too_late()));
                        (name, answer)
                    });
                }
                Err(message) => {
                    requests.spawn(async move { (name, Err(message)) });
                }
            }
        }
        let mut results = SourceResults::new();
        while let Some(joined) = requests.join_next().await {
            if let Ok((name, mut answer)) = joined {
                match &mut answer {
                    Ok(data) => self.secrets.redact_json(data),
                    Err(message) => {
                        *message = self.secrets.redact(message);
                        resolver.push(Problem {
                            message: format!("Data source `{name}`: {message}"),
                            path: Some(format!("/sources/{}", pointer_escape(&name))),
                        });
                    }
                }
                results.insert(name, answer);
            }
        }
        results
    }

    /// Fetches the document's images at once, each by its URL once.
    async fn fetch_images(
        &self,
        resolver: &mut Resolver,
        deadline: Instant,
    ) -> BTreeMap<String, Vec<u8>> {
        let mut fetches = tokio::task::JoinSet::new();
        for (key, url, path) in std::mem::take(&mut resolver.images) {
            let http = self.http.clone();
            fetches.spawn(async move {
                let bytes = tokio::time::timeout_at(deadline, fetch_image(&http, &url))
                    .await
                    .unwrap_or_else(|_| Err(too_late()));
                (key, url, path, bytes)
            });
        }
        let mut images = BTreeMap::new();
        while let Some(joined) = fetches.join_next().await {
            let Ok((key, url, path, bytes)) = joined else {
                continue;
            };
            match bytes {
                Ok(bytes) => {
                    images.insert(key, bytes);
                }
                Err(message) => resolver.push(Problem {
                    message: format!("Image {url}: {message}"),
                    path: Some(path),
                }),
            }
        }
        images
    }
}

/// The catalogue's model and palette of a display that can be drawn: the model
/// lists the palette, and the renderer writes the model's format with tokens for
/// the pair.
#[must_use]
pub fn drawable(model: &str, palette: &str) -> Option<(&'static Model, &'static Palette)> {
    let catalog = Catalog::get();
    let (model, palette) = (catalog.model(model)?, catalog.palette(palette)?);
    (catalog.supports(&model.name, &palette.id) && Renderer::supports(model, palette))
        .then_some((model, palette))
}

fn too_late() -> String {
    format!(
        "gave no answer in time: a screen's data and images are fetched within {} seconds",
        FETCH_TIME.as_secs()
    )
}

async fn fetch_image(http: &Outbound, url: &str) -> Result<Vec<u8>, String> {
    if url.starts_with("data:") {
        let data = data_url::DataUrl::process(url).map_err(|error| format!("{error:?}"))?;
        let (bytes, _) = data.decode_to_vec().map_err(|error| format!("{error:?}"))?;
        return Ok(bytes);
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("an image URL starts with http://, https:// or data:".to_owned());
    }
    http.read(http.client().get(url), IMAGE_BYTES).await
}

async fn fetch_json(http: &Outbound, request: reqwest::RequestBuilder) -> Result<Value, String> {
    let bytes = http.read(request, SOURCE_BYTES).await?;
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

/// Resolves a document's templates, collecting problems and the images to fetch,
/// within the drawing's bounds.
struct Resolver {
    templates: Arc<Environment<'static>>,
    problems: Vec<Problem>,
    /// Problems beyond [`MAX_PROBLEMS`], counted.
    dropped: usize,
    /// Key, URL and document path of each distinct image.
    images: Vec<(String, String, String)>,
    components: usize,
    started: std::time::Instant,
    /// The document went past a bound and the rest of it was left out.
    exhausted: bool,
}

impl Resolver {
    fn new(templates: Arc<Environment<'static>>) -> Self {
        Self {
            templates,
            problems: Vec::new(),
            dropped: 0,
            images: Vec::new(),
            components: 0,
            started: std::time::Instant::now(),
            exhausted: false,
        }
    }

    fn push(&mut self, problem: Problem) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(problem);
        } else {
            self.dropped += 1;
        }
    }

    fn problem(&mut self, path: &str, message: impl Into<String>) {
        self.push(Problem {
            message: message.into(),
            path: Some(path.to_owned()),
        });
    }

    /// The problems, with a last one counting those left out.
    fn finish(mut self) -> Vec<Problem> {
        if self.dropped > 0 {
            self.problems.push(Problem {
                message: format!("and {} more problems", self.dropped),
                path: None,
            });
        }
        self.problems
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

    /// A data source's request. It asks for JSON with `Accept` and labels a body as
    /// JSON with `Content-Type`, and a header the source names takes the place of
    /// the one of the same name, as the model's `Source` states.
    fn request(
        &mut self,
        client: &reqwest::Client,
        source: &Value,
        scope: &Template,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, String> {
        let url = self.text(&source["url"], scope, &format!("{path}/url"));
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            // The URL itself stays out of the message: it may hold a secret.
            return Err("its URL does not start with http:// or https://".to_owned());
        }
        let method = if source.get("method").and_then(Value::as_str) == Some("POST") {
            reqwest::Method::POST
        } else {
            reqwest::Method::GET
        };
        let body = source.get("body").filter(|body| body.is_string());
        let json = HeaderValue::from_static("application/json");
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, json.clone());
        if body.is_some() {
            headers.insert(CONTENT_TYPE, json);
        }
        let named = source.get("headers").and_then(Value::as_object);
        for (name, template) in named.into_iter().flatten() {
            let value = self.text(
                template,
                scope,
                &format!("{path}/headers/{}", pointer_escape(name)),
            );
            let header = HeaderName::try_from(name.as_str())
                .map_err(|_| format!("`{name}` is not a header name"))?;
            // The value itself stays out of the message: it may hold a secret.
            let value = HeaderValue::try_from(value).map_err(|_| {
                format!("its `{name}` header holds a control character, such as a line break")
            })?;
            headers.insert(header, value);
        }
        let request = client.request(method, url).headers(headers);
        Ok(match body {
            Some(body) => request.body(self.text(body, scope, &format!("{path}/body"))),
            None => request,
        })
    }

    fn document(&mut self, document: &Value, scope: &Template) -> Value {
        let mut resolved = Map::new();
        if let Some(bar) = document.get("titleBar") {
            let mut bar = self.fields(bar, scope, "/titleBar");
            self.take_image(&mut bar, "icon", "/titleBar/icon");
            resolved.insert("titleBar".to_owned(), bar);
        }
        let content = self.content(&document["content"], scope, "/content");
        resolved.insert("content".to_owned(), content);
        Value::Object(resolved)
    }

    /// Replaces the URL at `member` with the key of the image it names, or with
    /// nothing when it names none.
    fn take_image(&mut self, object: &mut Value, member: &str, path: &str) {
        let url = match object.get(member) {
            None | Some(Value::Null) => return,
            Some(Value::String(url)) if !url.is_empty() => url.clone(),
            Some(Value::String(_)) => {
                object[member] = Value::Null;
                return;
            }
            Some(other) => {
                self.problem(path, format!("an image URL is text, not {other}"));
                object[member] = Value::Null;
                return;
            }
        };
        if let Some((key, _, _)) = self.images.iter().find(|(_, known, _)| *known == url) {
            object[member] = Value::String(key.clone());
            return;
        }
        if self.images.len() == MAX_IMAGES {
            self.problem(
                path,
                format!("a screen shows at most {MAX_IMAGES} different images"),
            );
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
                        .filter(|child| !child.is_null())
                        .collect(),
                ),
                ("content", child) => self.content(child, scope, &here),
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

    /// Counts a component against the bounds; `false` once the document is past
    /// them, reported once.
    fn admit(&mut self, path: &str) -> bool {
        if self.exhausted {
            return false;
        }
        self.components += 1;
        if self.components > MAX_COMPONENTS {
            self.exhausted = true;
            self.problem(
                path,
                format!("the screen expands to more than {MAX_COMPONENTS} components; the rest is left out"),
            );
        } else if self.started.elapsed() > RESOLVE_TIME {
            self.exhausted = true;
            self.problem(
                path,
                format!(
                    "the screen's templates ran longer than {} seconds; the rest is left out",
                    RESOLVE_TIME.as_secs()
                ),
            );
        }
        !self.exhausted
    }

    /// A component where one must stand: nothing becomes an empty column.
    fn content(&mut self, node: &Value, scope: &Template, path: &str) -> Value {
        match self.component(node, scope, path) {
            Value::Null => empty(),
            component => component,
        }
    }

    /// The component resolved, or nothing (`Null`) for a `choose` that draws
    /// nothing.
    fn component(&mut self, node: &Value, scope: &Template, path: &str) -> Value {
        if !self.admit(path) {
            return empty();
        }
        let Some((kind, value)) = node.as_object().and_then(|object| object.iter().next()) else {
            self.problem(path, "a component is an object with one member");
            return empty();
        };
        let here = format!("{path}/{kind}");
        match kind.as_str() {
            "each" => self.each(value, scope, &here),
            "choose" => self.choose(value, scope, &here),
            "table" => self.table(value, scope, &here),
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

    /// A `table` keeps its settings and its columns' headings, and gets a row of
    /// cell values for each element, each cell counted as a component, and the
    /// line saying how many were left out.
    fn table(&mut self, table: &Value, scope: &Template, path: &str) -> Value {
        let (scopes, more) = self.elements(table, scope, path);
        let columns = table["columns"].as_array().cloned().unwrap_or_default();
        let mut resolved = table.as_object().cloned().unwrap_or_default();
        resolved.remove("items");
        resolved.remove("limit");
        let headings: Vec<Value> = columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let heading = self.text(
                    &column["heading"],
                    scope,
                    &format!("{path}/columns/{index}/heading"),
                );
                let mut column = column.as_object().cloned().unwrap_or_default();
                column.remove("text");
                column.insert("heading".to_owned(), Value::String(heading));
                Value::Object(column)
            })
            .collect();
        let mut rows = Vec::new();
        for scope in &scopes {
            // Each cell is drawn as a component is, so each counts as one.
            if !columns.iter().all(|_| self.admit(path)) {
                break;
            }
            let cells = columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    self.value(
                        column["text"].as_str().unwrap_or_default(),
                        scope,
                        &format!("{path}/columns/{index}/text"),
                    )
                })
                .collect();
            rows.push(Value::Array(cells));
        }
        resolved.insert("columns".to_owned(), Value::Array(headings));
        resolved.insert("rows".to_owned(), Value::Array(rows));
        resolved.insert("more".to_owned(), more.map_or(Value::Null, Value::String));
        json!({ "table": resolved })
    }

    /// A `choose` becomes its `then` where `when` is true as Jinja's `if` tests
    /// it, else its `else`; with no `else`, nothing, which its parent leaves out.
    fn choose(&mut self, choose: &Value, scope: &Template, path: &str) -> Value {
        let when = self.value(
            choose["when"].as_str().unwrap_or_default(),
            scope,
            &format!("{path}/when"),
        );
        let branch = if Template::from(Serde(&when)).is_true() {
            "then"
        } else {
            "else"
        };
        choose.get(branch).map_or(Value::Null, |component| {
            self.component(component, scope, &format!("{path}/{branch}"))
        })
    }

    /// The elements an `each` or a `table` draws, each in the scope its templates
    /// read it in as `item`, and the line saying how many its `limit` left out.
    fn elements(
        &mut self,
        node: &Value,
        scope: &Template,
        path: &str,
    ) -> (Vec<Template>, Option<String>) {
        let items = match self.value(
            node["items"].as_str().unwrap_or_default(),
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
        let limit = node
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(EACH_LIMIT, |limit| {
                usize::try_from(limit).unwrap_or(EACH_LIMIT)
            });
        let scopes = items
            .iter()
            .take(limit)
            .map(|item| context! { item => Template::from(Serde(item)), ..scope.clone() })
            .collect();
        let hidden = items.len().saturating_sub(limit);
        let more = (hidden > 0).then(|| {
            let scope = context! { hidden => hidden, ..scope.clone() };
            self.text(&node["more"], &scope, &format!("{path}/more"))
        });
        (scopes, more)
    }

    /// An `each` becomes a column or row of its content once per element, and a
    /// line saying how many were left out.
    fn each(&mut self, each: &Value, scope: &Template, path: &str) -> Value {
        let (scopes, more) = self.elements(each, scope, path);
        let mut children: Vec<Value> = scopes
            .iter()
            .map(|scope| self.component(&each["content"], scope, &format!("{path}/content")))
            .filter(|child| !child.is_null())
            .collect();
        if let Some(more) = more {
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
                "grow": each.get("grow").cloned().unwrap_or(json!(false)),
            }),
        );
        Value::Object(stack)
    }
}

/// A column of nothing, where a component cannot be drawn.
fn empty() -> Value {
    json!({ "column": { "children": [] } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver() -> Resolver {
        Resolver::new(Arc::new(templates()))
    }

    fn resolve(document: &Value, scope: &Template) -> (Value, Vec<Problem>) {
        let mut resolver = resolver();
        let screen = resolver.document(document, scope);
        (screen, resolver.finish())
    }

    #[test]
    fn the_resolver_walks_every_member_the_model_gives_components() {
        // The resolver finds components by member name; a member the model adds
        // that holds one would otherwise be read as text and escape the bounds.
        let contract: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../gen/management-api.openapi.json"
        )))
        .unwrap();
        let component = Value::from("#/components/schemas/Component");
        let holders: std::collections::BTreeSet<&str> = contract["components"]["schemas"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|schema| schema["properties"].as_object())
            .flatten()
            .filter(|(_, property)| {
                property["$ref"] == component || property["items"]["$ref"] == component
            })
            .map(|(member, _)| member.as_str())
            .collect();
        assert_eq!(holders, ["children", "content", "else", "then"].into());
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
    fn choose_draws_then_where_its_test_is_true_as_jinja_tests_it() {
        for (value, expected) in [
            (json!(false), "else"),
            (json!(null), "else"),
            (json!(0), "else"),
            (json!(""), "else"),
            (json!([]), "else"),
            (json!({}), "else"),
            (json!(true), "then"),
            (json!(1), "then"),
            (json!("no"), "then"),
            (json!([0]), "then"),
            (json!({ "a": 1 }), "then"),
        ] {
            let scope = context! { tested => Template::from(Serde(&value)) };
            let document = json!({ "content": { "choose": {
                "when": "{{ tested }}",
                "then": { "label": { "text": "then" } },
                "else": { "label": { "text": "else" } }
            } } });
            let (resolved, problems) = resolve(&document, &scope);
            assert_eq!(problems, [], "{value}");
            assert_eq!(resolved["content"]["label"]["text"], expected, "{value}");
        }
    }

    #[test]
    fn a_table_gets_its_headings_and_a_row_of_cells_for_each_element() {
        let scope = context! { stocks => Template::from(Serde(json!([
            { "symbol": "AMZN", "price": 184.2 },
            { "symbol": "MSFT", "price": 412.5 },
            { "symbol": "NVDA", "price": 121.9 }
        ]))) };
        let document = json!({ "content": { "table": {
            "items": "{{ stocks }}",
            "limit": 2,
            "more": "and {{ hidden }} more",
            "size": "small",
            "columns": [
                { "heading": "{{ 'Symbol' | upper }}", "text": "{{ item.symbol }}", "align": "start", "clamp": 1 },
                { "heading": "Price", "text": "{{ item.price }}", "align": "end", "clamp": 1 }
            ]
        } } });
        let (resolved, problems) = resolve(&document, &scope);
        assert_eq!(problems, []);
        assert_eq!(
            resolved["content"],
            json!({ "table": {
                "size": "small",
                "more": "and 1 more",
                "columns": [
                    { "heading": "SYMBOL", "align": "start", "clamp": 1 },
                    { "heading": "Price", "align": "end", "clamp": 1 }
                ],
                "rows": [["AMZN", 184.2], ["MSFT", 412.5]]
            } })
        );
    }

    #[test]
    fn every_table_cell_counts_against_the_bound() {
        let columns: Vec<Value> = (0..12)
            .map(|column| json!({ "heading": "H", "text": format!("{{{{ item }}}}{column}"), "align": "start", "clamp": 1 }))
            .collect();
        let document = json!({ "content": { "each": {
            "items": "{{ range(3) | list }}",
            "content": { "table": { "items": "{{ range(100) | list }}", "columns": columns } }
        } } });
        let (resolved, problems) = resolve(&document, &context! {});
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].message.contains("more than 2000 components"),
            "{problems:?}"
        );
        let rows: usize = resolved["content"]["column"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|table| table["table"]["rows"].as_array().map_or(0, Vec::len))
            .sum();
        assert!(rows * 12 <= MAX_COMPONENTS, "{rows} rows drawn");
    }

    #[test]
    fn a_choose_without_else_where_its_test_is_false_is_left_out() {
        let document = json!({ "content": { "row": { "children": [
            { "label": { "text": "kept" } },
            { "choose": { "when": "{{ false }}", "then": { "label": { "text": "then" } } } },
            { "each": {
                "items": "{{ [0, 1, 2] }}",
                "content": { "choose": {
                    "when": "{{ item == 1 }}",
                    "then": { "value": { "text": "{{ item }}" } }
                } }
            } }
        ] } } });
        let (resolved, problems) = resolve(&document, &context! {});
        assert_eq!(problems, []);
        let children = &resolved["content"]["row"]["children"];
        assert_eq!(children.as_array().unwrap().len(), 2);
        assert_eq!(children[0]["label"]["text"], "kept");
        assert_eq!(
            children[1]["column"]["children"],
            json!([{ "value": { "text": 1 } }])
        );

        let alone = json!({ "content": { "choose": {
            "when": "{{ 0 }}",
            "then": { "label": { "text": "then" } }
        } } });
        let (resolved, _) = resolve(&alone, &context! {});
        assert_eq!(resolved["content"], json!({ "column": { "children": [] } }));
    }

    #[test]
    fn multiply_scales_numbers_and_numeric_text_and_refuses_the_rest() {
        let document = json!({ "content": { "column": { "children": [
            { "lineChart": { "series": [ { "values": "{{ [150, '2.5'] | map('multiply', 0.01) | list }}" } ] } },
            { "title": { "text": "{{ 'many' | multiply(2) }}" } },
            { "title": { "text": "{{ 'unavailable' | multiply(0.01, default=0) }}" } },
            { "title": { "text": "{{ 'unavailable' | multiply(0.01, '?') }}" } }
        ] } } });
        let (resolved, problems) = resolve(&document, &context! {});
        assert_eq!(
            resolved["content"]["column"]["children"][0]["lineChart"]["series"][0]["values"],
            json!([1.5, 0.025])
        );
        let children = &resolved["content"]["column"]["children"];
        assert_eq!(children[2]["title"]["text"], 0);
        assert_eq!(children[3]["title"]["text"], "?");
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].path.as_deref(),
            Some("/content/column/children/1/title/text")
        );
    }

    #[test]
    fn an_each_that_grows_is_a_stack_that_grows() {
        let document = json!({ "content": { "each": {
            "items": "{{ [1, 2] }}",
            "direction": "row",
            "grow": true,
            "content": { "title": { "text": "{{ item }}" } }
        } } });
        let (resolved, problems) = resolve(&document, &context! {});
        assert_eq!(problems, []);
        assert_eq!(resolved["content"]["row"]["grow"], true);
        assert_eq!(
            resolved["content"]["row"]["children"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
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
        let mut resolver = resolver();
        let screen = resolver.document(&document, &context! {});
        assert_eq!(screen["titleBar"]["icon"], "image-0");
        assert_eq!(screen["content"]["image"]["url"], "image-1");
        assert_eq!(resolver.images[1].1, "data:image/png;base64,AA==");
    }

    #[test]
    fn an_image_named_twice_is_fetched_once_and_a_missing_one_is_none() {
        let scope = context! { photos => Template::from(Serde(json!([
            { "url": "https://photos.example.test/a.png" },
            { "url": "https://photos.example.test/a.png" },
            { "url": null },
            { "url": 7 }
        ]))) };
        let document = json!({ "content": { "each": {
            "items": "{{ photos }}",
            "content": { "image": { "url": "{{ item.url }}" } }
        } } });
        let mut resolver = resolver();
        let screen = resolver.document(&document, &scope);
        let children = &screen["content"]["column"]["children"];
        assert_eq!(children[0]["image"]["url"], "image-0");
        assert_eq!(children[1]["image"]["url"], "image-0");
        assert_eq!(children[2]["image"]["url"], Value::Null);
        assert_eq!(children[3]["image"]["url"], Value::Null);
        assert_eq!(resolver.images.len(), 1);
        let problems = resolver.finish();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(
            problems[0].path.as_deref(),
            Some("/content/each/content/image/url")
        );
    }

    #[test]
    fn an_each_without_a_limit_draws_at_most_a_hundred() {
        let document = json!({ "content": { "each": {
            "items": "{{ range(150) | list }}",
            "more": "and {{ hidden }} more",
            "content": { "label": { "text": "{{ item }}" } }
        } } });
        let (resolved, problems) = resolve(&document, &context! {});
        assert_eq!(problems, []);
        let children = resolved["content"]["column"]["children"]
            .as_array()
            .unwrap();
        assert_eq!(children.len(), EACH_LIMIT + 1);
        assert_eq!(children[EACH_LIMIT]["label"]["text"], "and 50 more");
    }

    #[test]
    fn a_document_stops_expanding_past_its_bound() {
        // A hundred times a hundred labels: five times the bound.
        let document = json!({ "content": { "each": {
            "items": "{{ range(100) | list }}",
            "content": { "each": {
                "items": "{{ range(100) | list }}",
                "content": { "label": { "text": "{{ item }}" } }
            } }
        } } });
        let (_, problems) = resolve(&document, &context! {});
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].message.contains("more than 2000 components"),
            "{problems:?}"
        );
    }

    #[test]
    fn problems_past_the_bound_are_counted() {
        let document = json!({ "content": { "each": {
            "items": "{{ range(50) | list }}",
            "content": { "value": { "text": "{{ 1 + }}" } }
        } } });
        let (_, problems) = resolve(&document, &context! {});
        assert_eq!(problems.len(), MAX_PROBLEMS + 1);
        assert_eq!(
            problems[MAX_PROBLEMS].message,
            format!("and {} more problems", 50 - MAX_PROBLEMS)
        );
    }

    #[test]
    fn single_expressions_are_recognised_only_whole() {
        assert_eq!(single_expression(" {{ a.b }} "), Some(" a.b "));
        assert_eq!(single_expression("{{ a }} and {{ b }}"), None);
        assert_eq!(single_expression("{{ a }}°"), None);
    }
}
