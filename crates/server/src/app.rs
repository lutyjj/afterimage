//! The server assembled from its settings: the stores over one database, the
//! device API, the management API, its MCP endpoint and the web UI behind one
//! router.

use std::path::Path;
use std::sync::Arc;

use std::convert::Infallible;

use afterimage_device_api::server::body::{BoxBody, boxed};
use afterimage_mcp::{Api, Catalog, Endpoint};
use bytes::Bytes;
use http::header::{CACHE_CONTROL, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use http::{HeaderValue, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use tower::steer::Steer;
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt, service_fn};
use tower_http::services::{ServeDir, ServeFile};

use crate::accounts::Accounts;
use crate::clock::Clock;
use crate::config::Config;
use crate::devices::Devices;
use crate::drawing::Drawing;
use crate::firmware::Firmware;
use crate::images::Images;
use crate::keys::Entropy;
use crate::oidc::Oidc;
use crate::playlists::Playlists;
use crate::registrations::Registrations;
use crate::screens::Screens;
use crate::secrets::Secrets;
use crate::sessions::{CookiePolicy, Sessions};
use crate::telemetry::{self, Saved, Telemetry};
use crate::time_zone::TimeZone;
use crate::tokens::Tokens;
use crate::{database, device_api, management_api};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] database::OpenError),
    #[error("the data directory: {0}")]
    DataDir(std::io::Error),
    #[error("{0}")]
    Settings(String),
}

/// The management API's contract as this build serves it, from `tools/generate`,
/// whose operations tagged `mcp` are the MCP endpoint's tools.
const CONTRACT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../gen/management-api.openapi.json"
));

/// The broker to publish to: the deployment's, or else the one the
/// administrator saved, or none until one is saved. A saved broker the server
/// no longer reads, such as an address with a query that an earlier release
/// took, leaves publishing off, so the web UI that corrects it still starts.
async fn broker(config: &Config, saved: &Saved) -> Result<Option<telemetry::Settings>, Error> {
    if let Some(settings) = config.mqtt() {
        return Ok(Some(settings));
    }
    let Some(saved) = saved
        .get()
        .await
        .map_err(|error| Error::Settings(format!("the saved MQTT broker: {error}")))?
    else {
        return Ok(None);
    };
    Ok(saved
        .settings(config.public_url.clone())
        .inspect_err(|reason| {
            tracing::warn!(
                %reason,
                "the saved MQTT broker's address cannot be used; publishing stays off until the administrator saves another"
            );
        })
        .ok())
}

/// The zone the settings name, or else the host's. Schedule times and dates on
/// screens are in it, and the web UI shows times in it, so it needs a name.
fn time_zone(config: &Config) -> Result<TimeZone, Error> {
    match config.time_zone.as_deref() {
        Some(name) => TimeZone::named(name)
            .map_err(|error| Error::Settings(format!("the time zone {name}: {error}"))),
        None => TimeZone::system().ok_or_else(|| {
            Error::Settings("the host's time zone has no name; set the time zone".to_owned())
        }),
    }
}

/// Opens the database under the data directory and builds the router.
///
/// # Errors
///
/// When the data directory or the database cannot be opened, or the settings
/// contradict each other.
///
/// # Panics
///
/// When the contract this build embeds tags an operation the MCP endpoint cannot
/// serve, which every test that builds the server catches.
pub async fn build(
    config: &Config,
    secrets: Secrets,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
) -> Result<App, Error> {
    let oidc_settings = config.oidc();
    if !config.password_sign_in && oidc_settings.is_none() {
        return Err(Error::Settings(
            "turning password sign-in off needs an OIDC provider".to_owned(),
        ));
    }
    let time_zone = time_zone(config)?;
    std::fs::create_dir_all(&config.data_dir).map_err(Error::DataDir)?;
    let pool = database::open(&config.data_dir.join("afterimage.db")).await?;
    let oidc = match (oidc_settings, config.public_url.as_deref()) {
        (Some(settings), Some(public_url)) => Some(
            Oidc::new(
                settings,
                public_url,
                pool.clone(),
                Arc::clone(&clock),
                Arc::clone(&entropy),
            )
            .map_err(Error::Settings)?,
        ),
        _ => None,
    };
    let secret_names = secrets.names();
    let drawing = Arc::new(Drawing::new(secrets, time_zone.clone()));
    let images = Arc::new(Images::new(config.data_dir.join("images")));
    let firmware = Arc::new(Firmware::new(
        &config.trmnl_url,
        config.cache_dir().join("firmware"),
        pool.clone(),
        Arc::clone(&clock),
    ));
    // Device state over MQTT.
    let saved_broker = Saved::new(pool.clone());
    let telemetry = Telemetry::start(
        broker(config, &saved_broker).await?,
        Devices::new(pool.clone(), Arc::clone(&clock)),
        Screens::new(pool.clone(), Arc::clone(&clock)),
        Playlists::new(pool.clone()),
        Arc::clone(&firmware),
        Arc::clone(&images),
        Arc::clone(&clock),
    )
    .map_err(|error| Error::Settings(format!("the MQTT broker: {error}")))?;
    let device_state = Arc::new(device_api::State {
        registrations: Registrations::new(pool.clone(), Arc::clone(&clock), Arc::clone(&entropy)),
        devices: Devices::new(pool.clone(), Arc::clone(&clock)),
        screens: Screens::new(pool.clone(), Arc::clone(&clock)),
        playlists: Playlists::new(pool.clone()),
        drawing: Arc::clone(&drawing),
        images: Arc::clone(&images),
        firmware: Arc::clone(&firmware),
        telemetry: telemetry.clone(),
        clock: Arc::clone(&clock),
    });
    let state = Arc::new(management_api::State {
        accounts: Accounts::new(pool.clone(), Arc::clone(&clock)),
        sessions: Arc::new(Sessions::new(
            pool.clone(),
            Arc::clone(&clock),
            Arc::clone(&entropy),
        )),
        tokens: Arc::new(Tokens::new(
            pool.clone(),
            Arc::clone(&clock),
            Arc::clone(&entropy),
        )),
        devices: Devices::new(pool.clone(), Arc::clone(&clock)),
        screens: Screens::new(pool.clone(), Arc::clone(&clock)),
        playlists: Playlists::new(pool),
        oidc,
        landing: config.landing(),
        cookies: CookiePolicy::new(config.public_url_is_https()),
        password_sign_in: config.password_sign_in,
        time_zone: time_zone.name().to_owned(),
        secret_names,
        drawing,
        images,
        firmware,
        telemetry,
        deployment_broker: config.mqtt(),
        saved_broker,
        broker_change: tokio::sync::Mutex::new(()),
        public_url: config.public_url.clone(),
        clock,
    });
    let management = management_api::service(state);
    let catalog = Catalog::from_openapi(CONTRACT, "GetAccount")
        .expect("the contract this build embeds holds the MCP endpoint's tools");
    let mcp = Endpoint::new(catalog, in_process(management.clone()));
    Ok(router(
        device_api::service(device_state),
        management,
        mcp,
        config.web_dir.as_deref(),
    ))
}

/// The management API as the MCP endpoint reaches it: whole bodies, no socket.
fn in_process<M>(management: M) -> Api
where
    M: Service<Request<BoxBody>, Response = Response<BoxBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    M::Future: Send + 'static,
{
    Api::new(service_fn(move |request: Request<Bytes>| {
        let management = management.clone();
        async move {
            let response = management
                .oneshot(request.map(|body| boxed(Full::new(body))))
                .await?;
            let (parts, body) = response.into_parts();
            Ok(match body.collect().await {
                Ok(body) => Response::from_parts(parts, body.to_bytes()),
                Err(error) => {
                    tracing::error!(%error, "an MCP call's answer could not be read");
                    let mut failed =
                        Response::new(Bytes::from_static(b"the answer could not be read"));
                    *failed.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    failed
                }
            })
        }
    }))
}

/// The whole server as one service: what `serve` runs and the tests call.
pub type App = BoxCloneService<Request<Incoming>, Response<BoxBody>, Infallible>;

/// Sends `/api/` to the device API, `/manage/` to the management API, `/mcp` to
/// its MCP endpoint, `/assets/` to the web UI's built assets, and every other
/// path to the web UI: a file, or its shell for a client route.
fn router<D, M>(device: D, management: M, mcp: Endpoint, web_dir: Option<&Path>) -> App
where
    D: Service<Request<BoxBody>, Response = Response<BoxBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    D::Future: Send + 'static,
    M: Service<Request<BoxBody>, Response = Response<BoxBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    M::Future: Send + 'static,
{
    let boxed_request = |request: Request<Incoming>| request.map(boxed);
    let (assets, shell) = match web_dir {
        Some(directory) => (assets(directory), shell(directory)),
        None => (not_found(), not_found()),
    };
    let services = vec![
        BoxCloneService::new(device.map_request(boxed_request)),
        BoxCloneService::new(management.map_request(boxed_request)),
        BoxCloneService::new(ServiceExt::<Request<Incoming>>::map_response(
            mcp,
            |response| response.map(boxed),
        )),
        assets,
        shell,
    ];
    BoxCloneService::new(Steer::new(
        services,
        |request: &Request<Incoming>, _: &[_]| match request.uri().path() {
            path if path.starts_with("/api/") => 0,
            path if path.starts_with("/manage/") => 1,
            "/mcp" => 2,
            path if path.starts_with("/assets/") => 3,
            _ => 4,
        },
    ))
}

type Web = BoxCloneService<Request<Incoming>, Response<BoxBody>, Infallible>;

/// The build's assets, which Vite names by a hash of their content: a name
/// never changes what it holds, so browsers keep them, and a missing one is
/// missing rather than the shell.
fn assets(directory: &Path) -> Web {
    BoxCloneService::new(ServiceExt::<Request<Incoming>>::map_response(
        ServeDir::new(directory),
        |mut response| {
            if response.status().is_success() {
                response.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            }
            response.map(boxed)
        },
    ))
}

/// The shell and every other unhashed file, sent whole on every request. The
/// Nix store gives every file one modification time, from which `ServeDir`
/// builds its validators, so a validator would call an upgraded shell
/// unchanged and leave the browser on a shell whose assets are gone.
fn shell(directory: &Path) -> Web {
    let files = ServeDir::new(directory).fallback(ServeFile::new(directory.join("index.html")));
    BoxCloneService::new(
        files
            .map_request(|mut request: Request<Incoming>| {
                request.headers_mut().remove(IF_NONE_MATCH);
                request.headers_mut().remove(IF_MODIFIED_SINCE);
                request
            })
            .map_response(|mut response| {
                let headers = response.headers_mut();
                headers.remove(ETAG);
                headers.remove(LAST_MODIFIED);
                headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                response.map(boxed)
            }),
    )
}

fn not_found() -> Web {
    BoxCloneService::new(service_fn(|_: Request<Incoming>| async {
        let mut response = Response::new(boxed(String::new()));
        *response.status_mut() = StatusCode::NOT_FOUND;
        Ok::<_, Infallible>(response)
    }))
}
