//! What the deployment gives every account.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};
use afterimage_render_typst::Renderer;
use afterimage_trmnl::catalog::Catalog;

use super::wire::fit;
use super::{Caller, Shared};

pub async fn get_server(
    _input: input::GetServerInput,
    Extension(state): Shared,
    _caller: Caller,
) -> Result<output::GetServerOutput, error::GetServerError> {
    let (default_model, default_palette) =
        Catalog::get().default_display("", None, Renderer::supports);
    Ok(output::GetServerOutput {
        time_zone: state.time_zone.clone(),
        secret_names: state.secret_names.clone(),
        default_display: model::Display {
            model: fit(default_model.name.clone())?,
            palette: fit(default_palette.id.clone())?,
        },
    })
}
