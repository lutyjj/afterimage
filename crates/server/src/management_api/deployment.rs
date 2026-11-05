//! What the deployment gives every account.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, output};

use super::{Caller, Shared};

pub async fn get_server(
    _input: input::GetServerInput,
    Extension(state): Shared,
    _caller: Caller,
) -> Result<output::GetServerOutput, error::GetServerError> {
    Ok(output::GetServerOutput {
        time_zone: state.time_zone.clone(),
        secret_names: state.secret_names.clone(),
    })
}
