//! Display models, claiming devices, their settings and playlists.

use afterimage_management_api::model::DeviceName;
use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, output};

use super::{Caller, Shared, internal, wire};
use crate::devices::{self, Changes, Claim};

fn not_found() -> error::NotFound {
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
    Ok(output::ListDevicesOutput {
        devices: devices
            .into_iter()
            .map(wire::device)
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
        device: wire::device(device)?,
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
        device: wire::device(device)?,
    })
}

pub async fn update_device(
    input: input::UpdateDeviceInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::UpdateDeviceOutput, error::UpdateDeviceError> {
    let changes = Changes {
        name: input.name.map(DeviceName::into_inner),
        display: input.display.map(wire::display),
        refresh_minutes: input
            .refresh_minutes
            .map(|minutes| i64::from(minutes.into_inner())),
    };
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
    Ok(output::UpdateDeviceOutput {
        device: wire::device(device)?,
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
        device: wire::device(device)?,
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
    tracing::info!(device = *input.device_id.inner(), reset, "device removed");
    Ok(output::DeleteDeviceOutput {})
}
