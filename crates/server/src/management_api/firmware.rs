//! TRMNL's firmware boards and each device's install request.

use afterimage_management_api::model::FirmwareVersion;
use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};

use super::devices::{not_found, shown};
use super::wire::fit;
use super::{Caller, Shared, internal};
use crate::firmware::InstallError;

/// TRMNL's list could not be read.
pub(super) fn unavailable(failure: &str) -> error::UpstreamFailure {
    error::UpstreamFailure {
        message: format!("TRMNL's firmware list could not be read: it {failure}."),
    }
}

pub async fn list_firmware_boards(
    _input: input::ListFirmwareBoardsInput,
    Extension(state): Shared,
    _caller: Caller,
) -> Result<output::ListFirmwareBoardsOutput, error::ListFirmwareBoardsError> {
    let boards = state
        .firmware
        .boards()
        .await
        .map_err(|failure| unavailable(&failure))?;
    let boards = boards
        .iter()
        .map(|board| {
            Ok(model::FirmwareBoard {
                name: fit(board.name.clone())?,
                label: board.label.clone(),
                chip_family: board.chip_family.clone(),
                latest_version: board
                    .latest()
                    .map(|release| fit(release.version.clone()))
                    .transpose()?,
            })
        })
        .collect::<Result<_, error::InternalFailure>>()?;
    Ok(output::ListFirmwareBoardsOutput { boards })
}

pub async fn install_firmware(
    input: input::InstallFirmwareInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::InstallFirmwareOutput, error::InstallFirmwareError> {
    let id = *input.device_id.inner();
    let device = state
        .devices
        .get(caller.account.id, id)
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    let version = input.version.as_ref().map(FirmwareVersion::as_str);
    match state.firmware.install(&device, version).await {
        Ok(()) => {}
        Err(
            error @ (InstallError::NoBoard
            | InstallError::UnknownBoard(_)
            | InstallError::NoSuchRelease { .. }
            | InstallError::NoRelease(_)
            | InstallError::BoardChanged),
        ) => {
            return Err(error::Conflict {
                message: error.to_string(),
            }
            .into());
        }
        Err(InstallError::DeviceGone) => return Err(not_found().into()),
        Err(error @ (InstallError::List(_) | InstallError::Image { .. })) => {
            tracing::warn!(%error, device = id, "firmware not installed");
            return Err(error::UpstreamFailure {
                message: error.to_string(),
            }
            .into());
        }
        Err(error @ (InstallError::Storage(_) | InstallError::Cache(_))) => {
            return Err(internal(&error).into());
        }
    }
    let device = state
        .devices
        .get(caller.account.id, id)
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    state.telemetry.device(device.id);
    Ok(output::InstallFirmwareOutput {
        device: shown(&state, device)?,
    })
}

pub async fn cancel_firmware_update(
    input: input::CancelFirmwareUpdateInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::CancelFirmwareUpdateOutput, error::CancelFirmwareUpdateError> {
    let id = *input.device_id.inner();
    let found = state
        .devices
        .get(caller.account.id, id)
        .await
        .map_err(|error| internal(&error))?;
    if found.is_none() {
        return Err(not_found().into());
    }
    state
        .firmware
        .cancel(id)
        .await
        .map_err(|error| internal(&error))?;
    let device = state
        .devices
        .get(caller.account.id, id)
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    state.telemetry.device(device.id);
    Ok(output::CancelFirmwareUpdateOutput {
        device: shown(&state, device)?,
    })
}
