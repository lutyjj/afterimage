//! Screens, their previews, and the secrets their data sources may read.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};

use super::{Caller, Shared, internal, wire};
use crate::screens::TEMPLATE_NAMES;
use crate::{drawing, showing};

/// Refuses a data source named like something templates already read.
fn check(document: &model::ScreenDocument) -> Result<(), error::ValidationException> {
    let taken = document
        .sources
        .iter()
        .flat_map(|sources| sources.inner().keys())
        .find(|name| TEMPLATE_NAMES.contains(&name.as_str()));
    match taken {
        Some(name) => Err(error::ValidationException {
            message: format!(
                "A data source cannot be named `{}`: templates read that name already.",
                name.as_str()
            ),
            field_list: None,
        }),
        None => Ok(()),
    }
}

fn not_found() -> error::NotFound {
    error::NotFound {
        message: "This screen does not exist.".to_owned(),
    }
}

pub async fn list_screens(
    _input: input::ListScreensInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::ListScreensOutput, error::ListScreensError> {
    let screens = state
        .screens
        .list(caller.account.id)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::ListScreensOutput {
        screens: screens
            .into_iter()
            .map(wire::summary)
            .collect::<Result<_, _>>()?,
    })
}

pub async fn create_screen(
    input: input::CreateScreenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::CreateScreenOutput, error::CreateScreenError> {
    check(&input.document)?;
    let document = wire::stored_document(&input.document)?;
    let screen = state
        .screens
        .create(caller.account.id, input.name.as_str(), &document)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::CreateScreenOutput {
        screen: wire::screen(screen)?,
    })
}

pub async fn get_screen(
    input: input::GetScreenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::GetScreenOutput, error::GetScreenError> {
    let screen = state
        .screens
        .get(caller.account.id, *input.screen_id.inner())
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    Ok(output::GetScreenOutput {
        screen: wire::screen(screen)?,
    })
}

pub async fn update_screen(
    input: input::UpdateScreenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::UpdateScreenOutput, error::UpdateScreenError> {
    check(&input.document)?;
    let document = wire::stored_document(&input.document)?;
    let screen = state
        .screens
        .update(
            caller.account.id,
            *input.screen_id.inner(),
            input.name.as_str(),
            &document,
        )
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    Ok(output::UpdateScreenOutput {
        screen: wire::screen(screen)?,
    })
}

pub async fn delete_screen(
    input: input::DeleteScreenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::DeleteScreenOutput, error::DeleteScreenError> {
    let deleted = state
        .screens
        .delete(caller.account.id, *input.screen_id.inner())
        .await
        .map_err(|error| internal(&error))?;
    if !deleted {
        return Err(not_found().into());
    }
    Ok(output::DeleteScreenOutput {})
}

pub async fn preview_screen(
    input: input::PreviewScreenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::PreviewScreenOutput, error::PreviewScreenError> {
    check(&input.document)?;
    let display = wire::display(input.display);
    let Some((model, palette)) = drawing::drawable(&display.model, &display.palette) else {
        return Err(invalid_display(&display).into());
    };
    let name = match input.device_id {
        Some(id) => {
            state
                .devices
                .get(caller.account.id, *id.inner())
                .await
                .map_err(|error| internal(&error))?
                .ok_or_else(|| error::NotFound {
                    message: "This device does not exist.".to_owned(),
                })?
                .name
        }
        None => "Preview".to_owned(),
    };
    let document: serde_json::Value =
        serde_json::from_str(&wire::stored_document(&input.document)?)
            .map_err(|error| internal(&error))?;
    let drawn = state
        .drawing
        .draw(
            &document,
            &showing::context(&name, model),
            model,
            palette,
            state.clock.now(),
        )
        .await;
    let sources = drawn
        .sources
        .into_iter()
        .filter_map(|(source, answer)| {
            let source = model::Identifier::try_from(source).ok()?;
            let answer = match answer {
                Ok(data) => model::SourceResult::Data(wire::json(data)),
                Err(message) => model::SourceResult::Error(message),
            };
            Some((source, answer))
        })
        .collect();
    Ok(output::PreviewScreenOutput {
        image: drawn
            .image
            .map(|image| aws_smithy_types::Blob::new(image.bytes)),
        sources,
        problems: drawn
            .problems
            .into_iter()
            .map(|problem| model::Problem {
                message: problem.message,
                path: problem.path,
            })
            .collect(),
    })
}

fn invalid_display(display: &crate::devices::Display) -> error::ValidationException {
    error::ValidationException {
        message: format!(
            "{} with {} is not a display Afterimage can draw: TRMNL's catalogue does not list the pair, or its model takes an image format the renderer does not write.",
            display.model, display.palette
        ),
        field_list: None,
    }
}
