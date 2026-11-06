//! An MCP endpoint over the management API.
//!
//! The tools are the operations the model tags [`TAG`], read from the `OpenAPI`
//! document the model emits ([`Catalog`]). The crate holds no schema of its own
//! and reaches nothing of the server but the API: a tool call is a request to it
//! with the caller's own `Authorization`, so the API decides who is asking and
//! what they may do, as it does for any program. Cookies never cross, so only a
//! token opens the endpoint.

mod catalog;
mod handler;

use std::convert::Infallible;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{HeaderValue, Request, Response, header};
use http_body::Body;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

pub use catalog::{Catalog, CatalogError, TAG, Tool};

use handler::Tools;

/// The largest request the endpoint reads. A tool call that writes a screen is
/// a few kilobytes; the API's own limit for one is 256 KiB.
const MAX_REQUEST_BYTES: usize = 512 * 1024;

type Buffered = BoxCloneService<Request<Bytes>, Response<Bytes>, Infallible>;

/// The API, reached without a socket: a request with its whole body, answered
/// with its whole body.
#[derive(Clone)]
pub struct Api(Arc<Mutex<Buffered>>);

impl Api {
    pub fn new<S>(service: S) -> Self
    where
        S: Service<Request<Bytes>, Response = Response<Bytes>, Error = Infallible>
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
    {
        Self(Arc::new(Mutex::new(BoxCloneService::new(service))))
    }

    async fn call(&self, request: Request<Bytes>) -> Response<Bytes> {
        // A mutex only because the handler must be `Sync` and a boxed service
        // is not: each call clones the service out and drops the lock first.
        let service = self
            .0
            .lock()
            .expect("no call panics holding the lock")
            .clone();
        match service.oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        }
    }
}

/// The endpoint as a service, for the path the server mounts it at.
#[derive(Clone)]
pub struct Endpoint {
    catalog: Arc<Catalog>,
    api: Api,
    transport: StreamableHttpService<Tools, NeverSessionManager>,
}

impl Endpoint {
    #[must_use]
    pub fn new(catalog: Catalog, api: Api) -> Self {
        let catalog = Arc::new(catalog);
        let tools = Tools {
            catalog: Arc::clone(&catalog),
            api: api.clone(),
        };
        // Stateless JSON answers. Any `Host` is accepted: the guard against DNS
        // rebinding protects servers a browser page can reach without a
        // credential, and this one needs a token no page can read.
        let config = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .disable_allowed_hosts()
            .with_max_request_body_bytes(MAX_REQUEST_BYTES);
        Self {
            catalog,
            api,
            transport: StreamableHttpService::new(
                move || Ok(tools.clone()),
                Arc::new(NeverSessionManager::default()),
                config,
            ),
        }
    }

    /// The API's answer to the check operation with this credential.
    async fn check(&self, authorization: Option<HeaderValue>) -> Response<Bytes> {
        let mut check = Request::get(self.catalog.check.as_str())
            .body(Bytes::new())
            .expect("a path from the document is a URI");
        if let Some(authorization) = authorization {
            check
                .headers_mut()
                .insert(header::AUTHORIZATION, authorization);
        }
        self.api.call(check).await
    }
}

type Answer = Response<BoxBody<Bytes, Infallible>>;

impl<B> Service<Request<B>> for Endpoint
where
    B: Body + Send + 'static,
    B::Error: Display,
    B::Data: Send + 'static,
{
    type Response = Answer;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Answer, Infallible>> + Send>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let endpoint = self.clone();
        Box::pin(async move {
            // A credential the API refuses goes no further, with the API's own
            // answer and challenge: a client learns at connecting, not at
            // every tool.
            let authorization = request.headers().get(header::AUTHORIZATION).cloned();
            let check = endpoint.check(authorization).await;
            if !check.status().is_success() {
                return Ok(check.map(|body| Full::new(body).boxed()));
            }
            endpoint.transport.oneshot(request).await
        })
    }
}
