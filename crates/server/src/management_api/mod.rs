//! The generated management API, served over the stores.
//!
//! Every operation in [`NeedsSession`] passes through [`RequireSession`]: all of
//! them but the five that start a session. Handlers read the account from
//! [`Caller`], which only that plugin provides.

mod accounts;
mod deployment;
mod devices;
mod screens;
mod wire;

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::request::Parts;
use http::{Request, Response, StatusCode, header};
use afterimage_management_api::server::body::{BoxBody, boxed};
use afterimage_management_api::server::operation::OperationShape;
use afterimage_management_api::server::plugin::{HttpMarker, Plugin, Scoped};
use afterimage_management_api::server::request::FromParts;
use afterimage_management_api::server::response::IntoResponse;
use afterimage_management_api::server::{AddExtensionLayer, Extension};
use afterimage_management_api::{Management, ManagementConfig, error, scope};
use tower::Service;

use crate::accounts::{Account, Accounts};
use crate::clock::Clock;
use crate::devices::Devices;
use crate::drawing::Drawing;
use crate::images::Images;
use crate::oidc::Oidc;
use crate::screens::Screens;
use crate::sessions::{CookieName, CookiePolicy, Sessions};

/// What the handlers act on.
pub struct State {
    pub accounts: Accounts,
    pub sessions: Arc<Sessions>,
    pub devices: Devices,
    pub screens: Screens,
    pub oidc: Option<Oidc>,
    /// Where the web UI lives, for redirects after OIDC sign-in.
    pub landing: String,
    pub cookies: CookiePolicy,
    pub password_sign_in: bool,
    /// The IANA name of the time zone of playlists and dates on screens.
    pub time_zone: String,
    /// Names of the secrets data sources may read; the values stay with the renderer.
    pub secret_names: Vec<String>,
    pub drawing: Arc<Drawing>,
    pub images: Arc<Images>,
    pub clock: Arc<dyn Clock>,
}

/// The management API as a service. Requests carry the smithy runtime's boxed body.
///
/// # Panics
///
/// When an operation of the generated API has no handler here, which every test
/// that builds the service catches.
#[must_use]
pub fn service(
    state: Arc<State>,
) -> impl Service<
    Request<BoxBody>,
    Response = Response<BoxBody>,
    Error = Infallible,
    Future: Send + 'static,
> + Clone
+ Send
+ 'static {
    let require = RequireSession {
        sessions: Arc::clone(&state.sessions),
        cookies: state.cookies,
    };
    let config = ManagementConfig::builder()
        .layer(AddExtensionLayer::new(state.cookies))
        .layer(AddExtensionLayer::new(state))
        .http_plugin(Scoped::new::<NeedsSession>(require))
        .build();
    Management::builder(config)
        .get_sign_in_options(accounts::get_sign_in_options)
        .create_first_account(accounts::create_first_account)
        .sign_in(accounts::sign_in)
        .start_oidc_sign_in(accounts::start_oidc_sign_in)
        .complete_oidc_sign_in(accounts::complete_oidc_sign_in)
        .sign_out(accounts::sign_out)
        .get_account(accounts::get_account)
        .list_display_models(devices::list_display_models)
        .list_devices(devices::list_devices)
        .claim_device(devices::claim_device)
        .get_device(devices::get_device)
        .update_device(devices::update_device)
        .set_playlist(devices::set_playlist)
        .delete_device(devices::delete_device)
        .get_device_image(devices::get_device_image)
        .list_screens(screens::list_screens)
        .create_screen(screens::create_screen)
        .get_screen(screens::get_screen)
        .update_screen(screens::update_screen)
        .delete_screen(screens::delete_screen)
        .preview_screen(screens::preview_screen)
        .get_server(deployment::get_server)
        .build()
        .expect("every management API operation has a handler")
}

scope! {
    /// Every operation but those that start a session. Named by exclusion, so an
    /// operation added to the model needs a session unless someone names it here.
    struct NeedsSession {
        excludes: [GetSignInOptions, CreateFirstAccount, SignIn, StartOidcSignIn, CompleteOidcSignIn]
    }
}

/// The signed-in account, which [`RequireSession`] puts on the request.
#[derive(Debug, Clone)]
pub struct Caller {
    pub account: Account,
    /// The session's token, so that signing out can end it.
    pub token: String,
}

impl<Protocol> FromParts<Protocol> for Caller {
    type Rejection = WiredWithoutSession;

    fn from_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .remove::<Caller>()
            .ok_or(WiredWithoutSession)
    }
}

/// A handler asked for the caller on an operation outside [`NeedsSession`]: a
/// wiring mistake, not something a request can cause.
#[derive(Debug, thiserror::Error)]
#[error("this operation was wired without a session")]
pub struct WiredWithoutSession;

impl<Protocol> IntoResponse<Protocol> for WiredWithoutSession {
    fn into_response(self) -> Response<BoxBody> {
        tracing::error!("an operation asked for the caller without requiring a session");
        let mut response = Response::new(boxed(String::new()));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        response
    }
}

/// Whether this request reached the server over TLS, by [`CookiePolicy`].
#[derive(Debug, Clone, Copy)]
pub struct OverTls(pub bool);

impl<Protocol> FromParts<Protocol> for OverTls {
    type Rejection = Infallible;

    fn from_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        let policy = parts
            .extensions
            .get::<CookiePolicy>()
            .copied()
            .unwrap_or(CookiePolicy::new(None));
        Ok(Self(policy.secure(&parts.headers)))
    }
}

/// Refuses a request without a live session, and gives the operation its
/// [`Caller`].
#[derive(Clone)]
pub struct RequireSession {
    sessions: Arc<Sessions>,
    cookies: CookiePolicy,
}

impl<Ser, Op: OperationShape, S> Plugin<Ser, Op, S> for RequireSession {
    type Output = Authenticate<S>;

    fn apply(&self, inner: S) -> Self::Output {
        Authenticate {
            inner,
            sessions: Arc::clone(&self.sessions),
            cookies: self.cookies,
        }
    }
}

impl HttpMarker for RequireSession {}

#[derive(Clone)]
pub struct Authenticate<S> {
    inner: S,
    sessions: Arc<Sessions>,
    cookies: CookiePolicy,
}

type BoxFuture<E> = Pin<Box<dyn Future<Output = Result<Response<BoxBody>, E>> + Send>>;

impl<S, B> Service<Request<B>> for Authenticate<S>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = BoxFuture<S::Error>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let mut inner = self.inner.clone();
        let sessions = Arc::clone(&self.sessions);
        let secure = self.cookies.secure(request.headers());
        let token = request
            .headers()
            .get(header::COOKIE)
            .and_then(|cookie| cookie.to_str().ok())
            .and_then(|cookie| CookieName::Session.read(cookie, secure));
        Box::pin(async move {
            let Some(token) = token else {
                return Ok(unauthenticated("Sign in to use this."));
            };
            match sessions.account(&token).await {
                Ok(Some(account)) => {
                    request.extensions_mut().insert(Caller { account, token });
                    inner.call(request).await
                }
                Ok(None) => Ok(unauthenticated("This session has ended. Sign in again.")),
                Err(error) => {
                    tracing::error!(%error, "a session could not be read");
                    Ok(failure())
                }
            }
        })
    }
}

/// The model's `Unauthenticated` error as restJson1 writes it. Built here because
/// the handler it guards never runs.
fn unauthenticated(message: &str) -> Response<BoxBody> {
    let body = serde_json::json!({ "message": message }).to_string();
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-errortype", "Unauthenticated")
        .body(boxed(body))
        .expect("a status and two static headers")
}

/// The model's `InternalFailure`, for a session that could not be read: the
/// session may be fine, so the caller must not take it for a sign-out.
fn failure() -> Response<BoxBody> {
    let body = serde_json::json!({ "message": "The server could not complete the request." });
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-amzn-errortype", "InternalFailure")
        .body(boxed(body.to_string()))
        .expect("a status and two static headers")
}

/// Logs a failure and answers it as the model's `InternalFailure`.
fn internal(error: &dyn std::fmt::Display) -> error::InternalFailure {
    tracing::error!(%error, "management API request failed");
    error::InternalFailure {
        message: "The server could not complete the request.".to_owned(),
    }
}

/// The handlers' shared state.
type Shared = Extension<Arc<State>>;
