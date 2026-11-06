//! The MCP handler: lists the catalog and has the API answer each call.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use http::{Method, Response, header};
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
    Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
    Tool as Descriptor, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::Value;

use crate::Api;
use crate::catalog::{Catalog, Tool};

/// The most text one answer carries back, about 60 thousand tokens. A larger
/// answer is a question asked too broadly.
const MAX_TEXT_BYTES: usize = 256 * 1024;

/// How long a client may keep the tool list. It changes only with the server's
/// release, so this bounds how stale a client is after an upgrade.
const LIST_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Clone)]
pub(crate) struct Tools {
    pub(crate) catalog: Arc<Catalog>,
    pub(crate) api: Api,
}

fn descriptor(tool: &Tool) -> Descriptor {
    let mut descriptor = Descriptor::new(
        tool.name.clone(),
        tool.description.clone(),
        tool.input_schema.clone(),
    );
    let method = &tool.method;
    descriptor.annotations = Some(
        ToolAnnotations::new()
            .read_only(method == Method::GET)
            .destructive(method == Method::DELETE)
            .idempotent(method.is_idempotent())
            .open_world(false),
    );
    descriptor
}

fn failure(message: impl Into<String>) -> CallToolResponse {
    CallToolResult::error(vec![ContentBlock::text(message)]).into()
}

impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        let config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("afterimage", env!("CARGO_PKG_VERSION")));
        match &self.catalog.instructions {
            Some(instructions) => config.with_instructions(instructions.clone()),
            None => config,
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        // Private: the list is the same for everyone, but only behind a token.
        std::future::ready(Ok(ListToolsResult::with_all_items(
            self.catalog.tools().iter().map(descriptor).collect(),
        )
        .with_ttl_ms(LIST_TTL_MS)
        .with_cache_scope(CacheScope::Private)))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let tool = self.catalog.get(&request.name).ok_or_else(|| {
            ErrorData::invalid_params(format!("there is no tool named `{}`", request.name), None)
        })?;
        // The endpoint let the request in for this header, so it is there.
        let authorization = context
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.headers.get(header::AUTHORIZATION))
            .cloned()
            .ok_or_else(|| ErrorData::invalid_request("the request carries no credential", None))?;
        let call = match tool.request(&request.arguments.unwrap_or_default(), authorization) {
            Ok(call) => call,
            Err(message) => return Ok(failure(message)),
        };
        Ok(answer(tool, self.api.call(call).await))
    }
}

/// What the API answered, as content: JSON as text, and an image, alone or as a
/// member of the JSON, as an image.
fn answer(tool: &Tool, response: Response<Bytes>) -> CallToolResponse {
    let status = response.status();
    let mime = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = response.into_body();
    if !status.is_success() {
        return failure(format!("HTTP {status}: {}", String::from_utf8_lossy(&body)));
    }
    if mime.starts_with("image/") {
        return CallToolResult::success(vec![ContentBlock::image(STANDARD.encode(&body), mime)])
            .into();
    }
    if body.is_empty() {
        return CallToolResult::success(vec![ContentBlock::text("Done.")]).into();
    }

    let mut content = Vec::new();
    let mut text = body;
    if let Ok(Value::Object(mut members)) = serde_json::from_slice::<Value>(&text) {
        for name in &tool.binary_members {
            let image = members
                .get(name)
                .and_then(Value::as_str)
                .and_then(|encoded| STANDARD.decode(encoded).ok())
                .and_then(|bytes| image_type(&bytes).map(|mime| (bytes, mime)));
            if let Some((bytes, mime)) = image {
                members.remove(name);
                content.push(ContentBlock::image(STANDARD.encode(bytes), mime));
            }
        }
        text = Bytes::from(serde_json::to_vec(&members).expect("a JSON object serialises"));
    }
    if text.len() > MAX_TEXT_BYTES {
        return failure(format!(
            "the answer is over {MAX_TEXT_BYTES} bytes; ask for less"
        ));
    }
    content.insert(0, ContentBlock::text(String::from_utf8_lossy(&text)));
    CallToolResult::success(content).into()
}

/// The image format these bytes start with, of those an MCP client shows.
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    const SIGNATURES: [(&[u8], &str); 3] = [
        (b"\x89PNG\r\n\x1a\n", "image/png"),
        (b"\xff\xd8\xff", "image/jpeg"),
        (b"GIF8", "image/gif"),
    ];
    SIGNATURES
        .iter()
        .find(|(signature, _)| bytes.starts_with(signature))
        .map(|(_, mime)| *mime)
}
