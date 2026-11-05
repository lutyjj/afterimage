//! The generated device API, served over the registry.

use std::future::Future;
use std::sync::Arc;

use aws_smithy_http_server::{AddExtensionLayer, Extension};
use aws_smithy_types::{Document, Number};
use afterimage_device_api::{DeviceApi, DeviceApiConfig, error, input, output};
use afterimage_trmnl::{DisplayStatus, Readings, SetupStatus};
use tokio::net::TcpListener;

use crate::devices::{self, Refusal, Registry, Setup};

/// The `refresh_rate` sent with statuses the firmware answers with its own fast
/// poll (202 and 500); it is required, and 0 would mean no wait at all.
const RETRY_SECONDS: i32 = 60;
/// Log entries kept from one batch. The firmware stores 10 and sends them with the
/// new ones; more than this is not firmware and is dropped.
const LOG_ENTRIES: usize = 32;
/// Characters kept from one log field.
const LOG_FIELD_CHARS: usize = 512;

/// Shown under the friendly ID after setup.
const CLAIM_MESSAGE: &str = "Claim this device in Afterimage with this code.";

/// Serves the device API on `listener` until `shutdown` completes.
///
/// # Errors
///
/// When accepting connections fails.
///
/// # Panics
///
/// When an operation of the generated API has no handler here, which the
/// `device_api` tests catch.
pub async fn serve(
    listener: TcpListener,
    registry: Arc<Registry>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let config = DeviceApiConfig::builder()
        .layer(AddExtensionLayer::new(registry))
        .build();
    let app = DeviceApi::builder(config)
        .setup_device(setup_device)
        .get_display(get_display)
        .get_current_screen(get_current_screen)
        .create_device_log(create_device_log)
        .build()
        .expect("every device API operation has a handler");
    afterimage_device_api::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown)
        .await
}

async fn setup_device(
    input: input::SetupDeviceInput,
    Extension(registry): Extension<Arc<Registry>>,
) -> Result<output::SetupDeviceOutput, error::SetupDeviceError> {
    let setup = registry
        .setup(&input.id, &input.model)
        .await
        .map_err(|error| internal_failure(&error))?;
    Ok(match setup {
        Setup::Registered { key, friendly_id } => {
            tracing::info!(%friendly_id, model = input.model, "device set up");
            output::SetupDeviceOutput {
                status: SetupStatus::Registered.code(),
                api_key: Some(key),
                friendly_id: Some(friendly_id),
                image_url: None,
                message: Some(CLAIM_MESSAGE.to_owned()),
            }
        }
        Setup::Refused(refusal) => {
            tracing::info!(?refusal, mac = ?input.id, model = ?input.model, "setup refused");
            output::SetupDeviceOutput {
                status: SetupStatus::NotRegistered.code(),
                api_key: None,
                friendly_id: None,
                image_url: None,
                message: Some(refusal_message(refusal).to_owned()),
            }
        }
    })
}

/// Shown on the device's "not registered" screen.
fn refusal_message(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::InvalidMac => "Afterimage could not read this device's MAC address.",
        Refusal::InvalidModel => "Afterimage could not read this device's model.",
        Refusal::UnclaimedLimit => "Too many devices are waiting to be claimed in Afterimage.",
    }
}

async fn get_display(
    input: input::GetDisplayInput,
    Extension(registry): Extension<Arc<Registry>>,
) -> Result<output::GetDisplayOutput, error::GetDisplayError> {
    let readings = Readings::parse(
        input.battery_voltage.as_deref(),
        input.percent_charged.as_deref(),
        input.rssi.as_deref(),
        input.firmware_version.as_deref(),
    );
    let device = registry
        .check_in(&input.access_token, &readings)
        .await
        .map_err(|error| internal_failure(&error))?;
    if let Some(device) = &device {
        tracing::debug!(friendly_id = device.friendly_id, ?readings, "check-in");
    }
    Ok(output::GetDisplayOutput {
        status: display_status(device.as_ref()).code(),
        refresh_rate: RETRY_SECONDS,
        reset_firmware: false,
        update_firmware: false,
        image_url: None,
        filename: None,
        firmware_url: None,
        special_function: None,
        action: None,
    })
}

async fn get_current_screen(
    input: input::GetCurrentScreenInput,
    Extension(registry): Extension<Arc<Registry>>,
) -> Result<output::GetCurrentScreenOutput, error::GetCurrentScreenError> {
    let device = registry
        .find(&input.access_token)
        .await
        .map_err(|error| internal_failure(&error))?;
    Ok(output::GetCurrentScreenOutput {
        status: display_status(device.as_ref()).code(),
        refresh_rate: RETRY_SECONDS,
        image_url: None,
        filename: None,
        rendered_at: None,
    })
}

/// Every known device is unclaimed until accounts can claim them.
fn display_status(device: Option<&devices::KnownDevice>) -> DisplayStatus {
    match device {
        Some(_) => DisplayStatus::Unclaimed,
        None => DisplayStatus::Reset,
    }
}

async fn create_device_log(
    input: input::CreateDeviceLogInput,
    Extension(registry): Extension<Arc<Registry>>,
) -> Result<output::CreateDeviceLogOutput, error::CreateDeviceLogError> {
    let Some(device) = registry
        .find(&input.access_token)
        .await
        .map_err(|error| internal_failure(&error))?
    else {
        return Err(error::UnknownDevice {
            message: Some("No device holds this access token.".to_owned()),
        }
        .into());
    };
    for entry in input.logs.iter().take(LOG_ENTRIES) {
        emit(&device.friendly_id, entry);
    }
    if input.logs.len() > LOG_ENTRIES {
        tracing::warn!(
            friendly_id = device.friendly_id,
            dropped = input.logs.len() - LOG_ENTRIES,
            "device sent more log entries than firmware keeps"
        );
    }
    Ok(output::CreateDeviceLogOutput {})
}

/// Writes one firmware log entry to the server's log. Fields are written with their
/// `Debug` form, so device text cannot add lines or escape sequences to the log.
fn emit(friendly_id: &str, entry: &Document) {
    let Document::Object(fields) = entry else {
        tracing::warn!(friendly_id, "device log entry is not an object");
        return;
    };
    let text = |name: &str| match fields.get(name) {
        Some(Document::String(text)) => {
            Some(text.chars().take(LOG_FIELD_CHARS).collect::<String>())
        }
        _ => None,
    };
    let message = text("message").unwrap_or_default();
    let source = text("source_path");
    let line = match fields.get("source_line") {
        Some(Document::Number(Number::PosInt(line))) => Some(*line),
        _ => None,
    };
    match text("level").as_deref() {
        Some("error" | "fatal") => {
            tracing::error!(target: "device", friendly_id, ?source, ?line, ?message);
        }
        Some("warn") => tracing::warn!(target: "device", friendly_id, ?source, ?line, ?message),
        Some("debug") => tracing::debug!(target: "device", friendly_id, ?source, ?line, ?message),
        _ => tracing::info!(target: "device", friendly_id, ?source, ?line, ?message),
    }
}

fn internal_failure(error: &devices::Error) -> error::InternalFailure {
    tracing::error!(%error, "device API request failed");
    error::InternalFailure {
        message: Some("The server could not complete the request.".to_owned()),
    }
}
