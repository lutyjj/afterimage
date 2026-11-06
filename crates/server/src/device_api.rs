//! The generated device API, served over the registrations and, for claimed
//! devices, what they show.

use std::convert::Infallible;
use std::sync::Arc;

use aws_smithy_types::{Document, Number};
use http::{Request, Response};
use afterimage_device_api::server::body::BoxBody;
use afterimage_device_api::server::{AddExtensionLayer, Extension};
use afterimage_device_api::{DeviceApi, DeviceApiConfig, error, input, output};
use afterimage_trmnl::{DisplayStatus, Mac, Readings, SetupStatus, panel_size};
use tower::Service;

use crate::clock::Clock;
use crate::devices::Devices;
use crate::drawing::Drawing;
use crate::firmware::Firmware;
use crate::images::Images;
use crate::registrations::{CheckIn, Refusal, Registrations, Setup};
use crate::screens::Screens;
use crate::showing;

/// What the handlers act on.
pub struct State {
    pub registrations: Registrations,
    pub devices: Devices,
    pub screens: Screens,
    pub drawing: Arc<Drawing>,
    pub images: Arc<Images>,
    pub firmware: Arc<Firmware>,
    pub clock: Arc<dyn Clock>,
}

type Shared = Extension<Arc<State>>;

/// The `refresh_rate` sent with 202. The model requires one, but on 202 firmware
/// 1.8.18 follows its own fast-poll schedule instead (`fastPollSeconds` in
/// `refresh_interval.cpp`): every 5 seconds at first, backing off to hourly.
const RETRY_SECONDS: i32 = 60;
/// Log entries kept from one batch. The firmware stores 10 and sends them with the
/// new ones; more than this is not firmware and is dropped.
const LOG_ENTRIES: usize = 32;
/// Characters kept from one log field.
const LOG_FIELD_CHARS: usize = 512;

/// Shown under the friendly ID after setup. Messages fit the firmware's 128-byte
/// message buffer.
const CLAIM_MESSAGE: &str = "Claim this device in Afterimage with this code.";

/// The device API as a service. Requests carry the smithy runtime's boxed body.
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
    let config = DeviceApiConfig::builder()
        .layer(AddExtensionLayer::new(state))
        .build();
    DeviceApi::builder(config)
        .setup_device(setup_device)
        .get_display(get_display)
        .get_current_screen(get_current_screen)
        .create_device_log(create_device_log)
        .get_screen_image(get_screen_image)
        .get_firmware_image(get_firmware_image)
        .build()
        .expect("every device API operation has a handler")
}

async fn setup_device(
    input: input::SetupDeviceInput,
    Extension(state): Shared,
) -> Result<output::SetupDeviceOutput, error::SetupDeviceError> {
    let setup = state
        .registrations
        .setup(&input.id, &input.model)
        .await
        .map_err(|error| internal_failure(&error))?;
    Ok(match setup {
        Setup::Registered {
            key,
            friendly_id,
            model,
        } => {
            tracing::info!(%friendly_id, model, "device set up");
            output::SetupDeviceOutput {
                status: SetupStatus::Registered.code(),
                api_key: Some(key),
                friendly_id: Some(friendly_id),
                image_url: None,
                message: Some(CLAIM_MESSAGE.to_owned()),
            }
        }
        Setup::Refused(refusal) => {
            // The headers are untrusted text; only a parsed MAC reaches the log.
            let mac = input.id.parse::<Mac>().ok();
            tracing::info!(
                ?refusal,
                mac = mac.map(tracing::field::display),
                "setup refused"
            );
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

/// The answer to a check-in whose key the server does not know, or whose
/// registration no device holds yet. A key the server does not know gets the
/// unclaimed answer because the only other answer that moves the device, 500, wipes
/// its Wi-Fi settings. Such a device keeps showing a code the server has dropped and
/// never runs setup again, since it still holds a key; a reset at the device
/// recovers it, and the log names its MAC. An empty key, which the firmware sends
/// after a setup that failed, gets the same answer: the device shows an empty code
/// until its next wake runs setup again. A claimed device gets it too until it has
/// a screen to show.
fn waiting() -> output::GetDisplayOutput {
    output::GetDisplayOutput {
        status: DisplayStatus::Unclaimed.code(),
        refresh_rate: RETRY_SECONDS,
        reset_firmware: false,
        update_firmware: false,
        image_url: None,
        filename: None,
        firmware_url: None,
        special_function: None,
        action: None,
    }
}

async fn get_display(
    input: input::GetDisplayInput,
    Extension(state): Shared,
    Origin(origin): Origin,
) -> Result<output::GetDisplayOutput, error::GetDisplayError> {
    let readings = Readings::parse(
        input.battery_voltage.as_deref(),
        input.percent_charged.as_deref(),
        input.rssi.as_deref(),
        input.firmware_version.as_deref(),
    );
    let panel = panel_size(input.width.as_deref(), input.height.as_deref());
    let check_in = state
        .registrations
        .check_in(
            &input.access_token,
            &readings,
            panel,
            input.model.as_deref(),
        )
        .await
        .map_err(|error| internal_failure(&error))?;
    Ok(match check_in {
        CheckIn::Unknown => {
            // The header is untrusted text; only a parsed MAC reaches the log.
            let mac = input.id.as_deref().and_then(|id| id.parse::<Mac>().ok());
            tracing::warn!(
                mac = mac.map(tracing::field::display),
                "check-in with a key no registration holds"
            );
            waiting()
        }
        CheckIn::Unclaimed { friendly_id } => {
            tracing::debug!(friendly_id, ?readings, "check-in");
            waiting()
        }
        CheckIn::Claimed { registration_id } => {
            tracing::debug!(registration_id, ?readings, "check-in");
            let Some(device) = state
                .devices
                .holding(registration_id)
                .await
                .map_err(|error| internal_failure(&error))?
            else {
                return Ok(waiting());
            };
            let shown = showing::next(
                &device,
                &state.devices,
                &state.screens,
                &state.drawing,
                &state.images,
                state.clock.now(),
            )
            .await
            .map_err(|error| internal_failure(&error))?;
            // Firmware sends `special_function` only when it woke for one.
            let special_function =
                input.firmware_special_function.is_some() || input.special_function.is_some();
            let firmware = state
                .firmware
                .check_in(&device, special_function)
                .await
                .map_err(|error| internal_failure(&error))?;
            output::GetDisplayOutput {
                status: DisplayStatus::Show.code(),
                image_url: Some(format!("{origin}/api/images/{}", shown.image)),
                filename: Some(filename(&shown.image)),
                refresh_rate: i32::try_from(shown.wait).unwrap_or(i32::MAX),
                // With status 0, the only status on which firmware 1.6 reads
                // them. 1.8 draws the screen before it installs; 1.6 skips it.
                update_firmware: firmware.is_some(),
                firmware_url: firmware.map(|image| format!("{origin}/api/firmware-images/{image}")),
                ..waiting()
            }
        }
        CheckIn::Reset => {
            tracing::info!("erasing a removed device's settings");
            output::GetDisplayOutput {
                status: DisplayStatus::Reset.code(),
                reset_firmware: true,
                ..waiting()
            }
        }
    })
}

async fn get_current_screen(
    input: input::GetCurrentScreenInput,
    Extension(state): Shared,
    Origin(origin): Origin,
) -> Result<output::GetCurrentScreenOutput, error::GetCurrentScreenError> {
    let device = state
        .devices
        .with_key(&input.access_token)
        .await
        .map_err(|error| internal_failure(&error))?;
    let showing = device
        .as_ref()
        .and_then(|device| device.showing.clone().map(|showing| (device, showing)));
    Ok(match showing {
        Some((device, showing)) => output::GetCurrentScreenOutput {
            status: DisplayStatus::Show.code(),
            refresh_rate: i32::try_from(device.refresh_minutes * 60).unwrap_or(i32::MAX),
            image_url: Some(format!("{origin}/api/images/{}", showing.image)),
            filename: Some(filename(&showing.image)),
            rendered_at: Some(aws_smithy_types::DateTime::from_secs(showing.rendered_at)),
        },
        None => output::GetCurrentScreenOutput {
            status: DisplayStatus::Unclaimed.code(),
            refresh_rate: RETRY_SECONDS,
            image_url: None,
            filename: None,
            rendered_at: None,
        },
    })
}

async fn get_screen_image(
    input: input::GetScreenImageInput,
    Extension(state): Shared,
) -> Result<output::GetScreenImageOutput, error::GetScreenImageError> {
    let Some(device) = state
        .devices
        .with_key(&input.access_token)
        .await
        .map_err(|error| internal_failure(&error))?
    else {
        return Err(error::UnknownDevice {
            message: Some("No device holds this access token.".to_owned()),
        }
        .into());
    };
    // Any image the device still has, not only its current one: a check-in may
    // have named one that a retried check-in has since replaced.
    let not_found = || error::ImageNotFound {
        message: Some("This device has no image of that name.".to_owned()),
    };
    let image = state
        .images
        .read(device.id, &input.name)
        .map_err(|error| internal_failure(&error))?
        .ok_or_else(not_found)?;
    Ok(output::GetScreenImageOutput {
        content_type: "image/png".to_owned(),
        image: aws_smithy_types::Blob::new(image),
    })
}

async fn get_firmware_image(
    input: input::GetFirmwareImageInput,
    Extension(state): Shared,
) -> Result<output::GetFirmwareImageOutput, error::GetFirmwareImageError> {
    let image = state
        .firmware
        .image(input.name.as_str())
        .map_err(|error| internal_failure(&error))?
        .ok_or_else(|| error::FirmwareNotFound {
            message: Some("This server keeps no firmware image of that name.".to_owned()),
        })?;
    Ok(output::GetFirmwareImageOutput {
        content_type: "application/octet-stream".to_owned(),
        image: aws_smithy_types::Blob::new(image),
    })
}

/// The `filename` the firmware caches an image under: its name without the
/// extension, within the firmware's 31 characters.
fn filename(image: &str) -> String {
    image.trim_end_matches(".png").to_owned()
}

/// The scheme and host the device reached the server at, from `Host` and a
/// reverse proxy's `X-Forwarded-Proto`; image URLs use it so that a device
/// downloads from where it checks in.
pub struct Origin(pub String);

impl<Protocol> afterimage_device_api::server::request::FromParts<Protocol> for Origin {
    type Rejection = Infallible;

    fn from_parts(parts: &mut http::request::Parts) -> Result<Self, Self::Rejection> {
        let header = |name: http::header::HeaderName| {
            parts
                .headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let scheme = header(http::HeaderName::from_static("x-forwarded-proto"))
            .filter(|scheme| scheme == "https")
            .unwrap_or_else(|| "http".to_owned());
        let host = header(http::header::HOST)
            .filter(|host| !host.is_empty() && host.bytes().all(|b| b.is_ascii_graphic()))
            .unwrap_or_else(|| "localhost".to_owned());
        Ok(Self(format!("{scheme}://{host}")))
    }
}

async fn create_device_log(
    input: input::CreateDeviceLogInput,
    Extension(state): Shared,
) -> Result<output::CreateDeviceLogOutput, error::CreateDeviceLogError> {
    let Some(registration) = state
        .registrations
        .find(&input.access_token)
        .await
        .map_err(|error| internal_failure(&error))?
    else {
        return Err(error::UnknownDevice {
            message: Some("No registration holds this access token.".to_owned()),
        }
        .into());
    };
    write_log(&registration.friendly_id, &input.logs);
    Ok(output::CreateDeviceLogOutput {})
}

/// Writes a device's log batch to the server's log: at most [`LOG_ENTRIES`]
/// entries and [`LOG_FIELD_CHARS`] characters per field. Fields are written in
/// their `Debug` form, so device text cannot add lines or escape sequences.
fn write_log(friendly_id: &str, entries: &[Document]) {
    for entry in entries.iter().take(LOG_ENTRIES) {
        write_entry(friendly_id, entry);
    }
    if entries.len() > LOG_ENTRIES {
        tracing::warn!(
            friendly_id,
            dropped = entries.len() - LOG_ENTRIES,
            "device sent more log entries than firmware keeps"
        );
    }
}

fn write_entry(friendly_id: &str, entry: &Document) {
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

fn internal_failure(error: &dyn std::fmt::Display) -> error::InternalFailure {
    tracing::error!(%error, "device API request failed");
    error::InternalFailure {
        message: Some("The server could not complete the request.".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::Mutex;

    use super::*;

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The server's log lines while `write_log` handles `entries`.
    fn logged(entries: &[Document]) -> Vec<String> {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        tracing::subscriber::with_default(subscriber, || write_log("K7QM2X", entries));
        let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
        text.lines().map(str::to_owned).collect()
    }

    fn entry(message: &str) -> Document {
        Document::Object(HashMap::from([
            ("level".to_owned(), Document::String("info".to_owned())),
            ("message".to_owned(), Document::String(message.to_owned())),
        ]))
    }

    /// The device's message is the event's message, written in its `Debug` form.
    #[test]
    fn writes_one_escaped_line_per_entry() {
        let lines = logged(&[entry("first\nforged line\u{1b}[2J"), entry("second")]);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            lines[0].ends_with(r#""first\nforged line\u{1b}[2J""#),
            "{}",
            lines[0]
        );
        assert!(lines[1].ends_with(r#""second""#), "{}", lines[1]);
    }

    #[test]
    fn keeps_32_entries_and_counts_the_rest() {
        let entries: Vec<_> = (0..40).map(|index| entry(&index.to_string())).collect();
        let lines = logged(&entries);
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.contains(" device:"))
                .count(),
            32
        );
        assert!(lines.last().unwrap().contains("dropped=8"), "{lines:?}");
    }

    #[test]
    fn truncates_long_fields() {
        let lines = logged(&[entry(&"x".repeat(600))]);
        assert_eq!(lines[0].matches('x').count(), LOG_FIELD_CHARS);
    }
}
