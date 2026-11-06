//! The generated management API, served over the stores.
//!
//! Every operation in [`NeedsSession`] passes through [`RequireSession`]: all of
//! them but the five that start a session. It takes a personal API token from
//! `Authorization: Bearer`, or else the session cookie, and refuses a read-only
//! token any request but a read. Handlers read the account from [`Caller`], which
//! only that plugin provides.

mod accounts;
mod deployment;
mod devices;
mod firmware;
mod mqtt;
mod screens;
mod tokens;
mod wire;

use std::convert::Infallible;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use afterimage_management_api::server::body::{BoxBody, boxed};
use afterimage_management_api::server::operation::OperationShape;
use afterimage_management_api::server::plugin::{HttpMarker, Plugin, Scoped};
use afterimage_management_api::server::request::FromParts;
use afterimage_management_api::server::response::IntoResponse;
use afterimage_management_api::server::service::ServiceShape;
use afterimage_management_api::server::{AddExtensionLayer, Extension};
use afterimage_management_api::{Management, ManagementConfig, error, scope};
use http::request::Parts;
use http::{HeaderValue, Method, Request, Response, StatusCode, header};
use tower::Service;

use crate::accounts::{Account, Accounts};
use crate::clock::Clock;
use crate::devices::Devices;
use crate::drawing::Drawing;
use crate::firmware::Firmware;
use crate::images::Images;
use crate::oidc::Oidc;
use crate::screens::Screens;
use crate::sessions::{CookieName, CookiePolicy, Sessions};
use crate::telemetry::{self, Saved, Telemetry};
use crate::tokens::Tokens;

/// What the handlers act on.
pub struct State {
    pub accounts: Accounts,
    pub sessions: Arc<Sessions>,
    pub tokens: Arc<Tokens>,
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
    pub firmware: Arc<Firmware>,
    /// Told of every change to a device, for MQTT, and of the broker's settings.
    pub telemetry: Telemetry,
    /// The broker the deployment sets, which the web UI cannot change.
    pub deployment_broker: Option<telemetry::Settings>,
    /// The broker the administrator saved, used where the deployment sets none.
    pub saved_broker: Saved,
    /// Held while the broker's settings change, so changes take turns.
    pub broker_change: tokio::sync::Mutex<()>,
    /// The web UI's address, for each MQTT device's link back to its page.
    pub public_url: Option<String>,
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
        tokens: Arc::clone(&state.tokens),
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
        .list_firmware_boards(firmware::list_firmware_boards)
        .install_firmware(firmware::install_firmware)
        .cancel_firmware_update(firmware::cancel_firmware_update)
        .list_screens(screens::list_screens)
        .create_screen(screens::create_screen)
        .get_screen(screens::get_screen)
        .update_screen(screens::update_screen)
        .delete_screen(screens::delete_screen)
        .preview_screen(screens::preview_screen)
        .get_server(deployment::get_server)
        .get_mqtt_settings(mqtt::get_mqtt_settings)
        .update_mqtt_settings(mqtt::update_mqtt_settings)
        .create_access_token(tokens::create_access_token)
        .list_access_tokens(tokens::list_access_tokens)
        .revoke_access_token(tokens::revoke_access_token)
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
    pub credential: Credential,
}

/// What the request signed in with.
#[derive(Debug, Clone)]
pub enum Credential {
    /// A browser's session, by its token, so that signing out can end it.
    Session(String),
    /// A personal API token, whose read-only limit [`RequireSession`] applies.
    Token,
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
    tokens: Arc<Tokens>,
    cookies: CookiePolicy,
}

/// Every operation it guards lists the service's errors, so it refuses a request
/// with the operation's own error type, as the protocol writes it.
impl<Ser, Op, S> Plugin<Ser, Op, S> for RequireSession
where
    Ser: ServiceShape,
    Op: OperationShape,
    Op::Error: From<error::Unauthenticated>
        + From<error::Forbidden>
        + From<error::InternalFailure>
        + IntoResponse<Ser::Protocol>,
{
    type Output = Authenticate<S, Ser::Protocol, Op::Error>;

    fn apply(&self, inner: S) -> Self::Output {
        Authenticate {
            inner,
            sessions: Arc::clone(&self.sessions),
            tokens: Arc::clone(&self.tokens),
            cookies: self.cookies,
            refusal: PhantomData,
        }
    }
}

impl HttpMarker for RequireSession {}

pub struct Authenticate<S, Protocol, Refusal> {
    inner: S,
    sessions: Arc<Sessions>,
    tokens: Arc<Tokens>,
    cookies: CookiePolicy,
    refusal: PhantomData<fn() -> (Protocol, Refusal)>,
}

impl<S: Clone, Protocol, Refusal> Clone for Authenticate<S, Protocol, Refusal> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            sessions: Arc::clone(&self.sessions),
            tokens: Arc::clone(&self.tokens),
            cookies: self.cookies,
            refusal: PhantomData,
        }
    }
}

type BoxFuture<E> = Pin<Box<dyn Future<Output = Result<Response<BoxBody>, E>> + Send>>;

impl<S, B, Protocol, Refusal> Service<Request<B>> for Authenticate<S, Protocol, Refusal>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
    Refusal: From<error::Unauthenticated>
        + From<error::Forbidden>
        + From<error::InternalFailure>
        + IntoResponse<Protocol>,
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
        let tokens = Arc::clone(&self.tokens);
        let secure = self.cookies.secure(request.headers());
        // A request with a bearer `Authorization` is judged by it alone, so a
        // token's limits hold even when the request carries a session cookie
        // too. Another scheme, such as a reverse proxy's Basic, leaves the
        // cookie to decide.
        let authorization = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| bearer(value.to_str().unwrap_or_default()));
        // The model's ReadOnlyIsGet validator ties GET to @readonly for every
        // operation a token may call.
        let reads = matches!(*request.method(), Method::GET | Method::HEAD);
        let token = request
            .headers()
            .get(header::COOKIE)
            .and_then(|cookie| cookie.to_str().ok())
            .and_then(|cookie| CookieName::Session.read(cookie, secure));
        Box::pin(async move {
            if let Some(bearer) = authorization {
                let Bearer::Secret(secret) = bearer else {
                    return Ok(unauthenticated::<Protocol, Refusal>(
                        "Send a token as Authorization: Bearer and the token.",
                        INVALID_TOKEN,
                    ));
                };
                return match tokens.grant(&secret).await {
                    Ok(Some(grant)) if grant.read_only && !reads => {
                        Ok(refuse::<Protocol, Refusal>(error::Forbidden {
                            message: "A read-only token cannot change anything.".to_owned(),
                        }))
                    }
                    Ok(Some(grant)) => {
                        request.extensions_mut().insert(Caller {
                            account: grant.account,
                            credential: Credential::Token,
                        });
                        inner.call(request).await
                    }
                    Ok(None) => Ok(unauthenticated::<Protocol, Refusal>(
                        "This token is not one of this server's. It may have been revoked.",
                        INVALID_TOKEN,
                    )),
                    Err(error) => Ok(refuse::<Protocol, Refusal>(internal(&error))),
                };
            }
            let Some(token) = token else {
                return Ok(unauthenticated::<Protocol, Refusal>(
                    "Sign in to use this.",
                    BEARER,
                ));
            };
            match sessions.account(&token).await {
                Ok(Some(account)) => {
                    request.extensions_mut().insert(Caller {
                        account,
                        credential: Credential::Session(token),
                    });
                    inner.call(request).await
                }
                Ok(None) => Ok(unauthenticated::<Protocol, Refusal>(
                    "This session has ended. Sign in again.",
                    BEARER,
                )),
                // The session may be fine, so the caller must not take this for
                // a sign-out.
                Err(error) => Ok(refuse::<Protocol, Refusal>(internal(&error))),
            }
        })
    }
}

/// An `Authorization` value of the bearer scheme.
enum Bearer {
    Secret(String),
    /// The scheme with no secret after it.
    Empty,
}

/// The bearer credential of an `Authorization` value, whose scheme name is
/// case-insensitive (RFC 9110, section 11.1); `None` for another scheme.
fn bearer(value: &str) -> Option<Bearer> {
    let value = value.trim();
    let (scheme, secret) = value.split_once(' ').unwrap_or((value, ""));
    let secret = secret.trim();
    scheme.eq_ignore_ascii_case("bearer").then(|| {
        if secret.is_empty() {
            Bearer::Empty
        } else {
            Bearer::Secret(secret.to_owned())
        }
    })
}

/// The bearer challenge RFC 6750 asks of a 401, and the one for a request whose
/// token is not one.
const BEARER: HeaderValue = HeaderValue::from_static("Bearer");
const INVALID_TOKEN: HeaderValue = HeaderValue::from_static("Bearer error=\"invalid_token\"");

/// The operation's answer for an error the plugin raises before its handler runs.
fn refuse<Protocol, Refusal: IntoResponse<Protocol>>(
    error: impl Into<Refusal>,
) -> Response<BoxBody> {
    error.into().into_response()
}

/// The model's `Unauthenticated`, with the challenge the bearer scheme adds to it.
fn unauthenticated<Protocol, Refusal>(message: &str, challenge: HeaderValue) -> Response<BoxBody>
where
    Refusal: From<error::Unauthenticated> + IntoResponse<Protocol>,
{
    let mut response = refuse::<Protocol, Refusal>(error::Unauthenticated {
        message: message.to_owned(),
    });
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, challenge);
    response
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
