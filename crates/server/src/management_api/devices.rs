//! Display models, claiming devices, their settings and playlists.

use afterimage_management_api::model::{self, DeviceName, FirmwareBoardName};
use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, output};

use super::firmware::unavailable;
use super::{Caller, Shared, State, internal, wire};
use crate::devices::{self, Changes, Claim, Device};

/// The device as the API shows it, with what TRMNL's firmware list says of its
/// board. A list that cannot be read leaves that out.
pub(super) async fn shown(
    state: &State,
    device: Device,
) -> Result<model::Device, error::InternalFailure> {
    let boards = state.firmware.boards().await.ok();
    wire::device(device, boards.as_deref())
}

pub(super) fn not_found() -> error::NotFound {
    error::NotFound {
        message: "This device does not exist.".to_owned(),
    }
}

/// The model's validation error, for a request that is well formed but names
/// something the server cannot use.
fn invalid(message: String) -> error::ValidationException {
    error::ValidationException {
        message,
        field_list: None,
    }
}

pub async fn list_display_models(
    _input: input::ListDisplayModelsInput,
) -> Result<output::ListDisplayModelsOutput, error::ListDisplayModelsError> {
    Ok(output::ListDisplayModelsOutput {
        models: wire::display_models()?,
    })
}

pub async fn list_devices(
    _input: input::ListDevicesInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::ListDevicesOutput, error::ListDevicesError> {
    let devices = state
        .devices
        .list(caller.account.id)
        .await
        .map_err(|error| internal(&error))?;
    let boards = state.firmware.boards().await.ok();
    Ok(output::ListDevicesOutput {
        devices: devices
            .into_iter()
            .map(|device| wire::device(device, boards.as_deref()))
            .collect::<Result<_, _>>()?,
    })
}

pub async fn claim_device(
    input: input::ClaimDeviceInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::ClaimDeviceOutput, error::ClaimDeviceError> {
    let claim = state
        .devices
        .claim(
            caller.account.id,
            input.code.as_str(),
            input.name.as_ref().map(DeviceName::as_str),
        )
        .await
        .map_err(|error| internal(&error))?;
    let (device, replaced) = match claim {
        Claim::NoSuchCode => {
            return Err(error::NotFound {
                message: "No device waiting to be claimed shows this code.".to_owned(),
            }
            .into());
        }
        Claim::Claimed(device) => (device, false),
        Claim::Replaced(device) => (device, true),
    };
    tracing::info!(device = device.id, replaced, "device claimed");
    Ok(output::ClaimDeviceOutput {
        device: shown(&state, device).await?,
        replaced,
    })
}

pub async fn get_device(
    input: input::GetDeviceInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::GetDeviceOutput, error::GetDeviceError> {
    let device = state
        .devices
        .get(caller.account.id, *input.device_id.inner())
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    Ok(output::GetDeviceOutput {
        device: shown(&state, device).await?,
    })
}

pub async fn update_device(
    input: input::UpdateDeviceInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::UpdateDeviceOutput, error::UpdateDeviceError> {
    if let Some(board) = &input.firmware_board {
        let boards = state
            .firmware
            .boards()
            .await
            .map_err(|failure| unavailable(&failure))?;
        if !boards.iter().any(|listed| listed.name == board.as_str()) {
            return Err(
                invalid(format!("TRMNL lists no firmware board {}.", board.as_str())).into(),
            );
        }
    }
    let changes = Changes {
        name: input.name.map(DeviceName::into_inner),
        display: input.display.map(wire::display),
        refresh_minutes: input
            .refresh_minutes
            .map(|minutes| i64::from(minutes.into_inner())),
        firmware_board: input.firmware_board.map(FirmwareBoardName::into_inner),
    };
    let board_chosen = changes.firmware_board.is_some();
    let device = match state
        .devices
        .update(caller.account.id, *input.device_id.inner(), changes)
        .await
    {
        Ok(device) => device.ok_or_else(not_found)?,
        Err(error @ devices::Error::UnsupportedDisplay { .. }) => {
            return Err(invalid(error.to_string()).into());
        }
        Err(error) => return Err(internal(&error).into()),
    };
    if board_chosen {
        state.firmware.forget_unused().await;
    }
    Ok(output::UpdateDeviceOutput {
        device: shown(&state, device).await?,
    })
}

pub async fn set_playlist(
    input: input::SetPlaylistInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::SetPlaylistOutput, error::SetPlaylistError> {
    let playlist: Vec<_> = input
        .playlist
        .inner()
        .iter()
        .map(wire::stored_entry)
        .collect();
    let device = match state
        .devices
        .set_playlist(caller.account.id, *input.device_id.inner(), &playlist)
        .await
    {
        Ok(device) => device.ok_or_else(not_found)?,
        Err(error @ devices::Error::UnknownScreen(_)) => {
            return Err(invalid(error.to_string()).into());
        }
        Err(error) => return Err(internal(&error).into()),
    };
    Ok(output::SetPlaylistOutput {
        device: shown(&state, device).await?,
    })
}

pub async fn delete_device(
    input: input::DeleteDeviceInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::DeleteDeviceOutput, error::DeleteDeviceError> {
    let reset = input.reset.unwrap_or(false);
    let deleted = state
        .devices
        .delete(caller.account.id, *input.device_id.inner(), reset)
        .await
        .map_err(|error| internal(&error))?;
    if !deleted {
        return Err(not_found().into());
    }
    if let Err(error) = state.images.forget(*input.device_id.inner()) {
        tracing::warn!(%error, "a removed device's images stay on disk");
    }
    // Its install request went with it.
    state.firmware.forget_unused().await;
    tracing::info!(device = *input.device_id.inner(), reset, "device removed");
    Ok(output::DeleteDeviceOutput {})
}

pub async fn get_device_image(
    input: input::GetDeviceImageInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::GetDeviceImageOutput, error::GetDeviceImageError> {
    let device = state
        .devices
        .get(caller.account.id, *input.device_id.inner())
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    let no_image = || error::NotFound {
        message: "This device shows no image yet.".to_owned(),
    };
    let showing = device.showing.ok_or_else(no_image)?;
    let image = state
        .images
        .read(device.id, &showing.image)
        .map_err(|error| internal(&error))?
        .ok_or_else(no_image)?;
    Ok(output::GetDeviceImageOutput {
        content_type: "image/png".to_owned(),
        image: aws_smithy_types::Blob::new(image),
    })
}
