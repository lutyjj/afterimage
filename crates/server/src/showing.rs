//! What a claimed device shows at a check-in: the next current entry of its
//! playlist, drawn now, or one of the server's own screens.
//!
//! A screen that cannot be drawn as written (a data source down, a template
//! error) leaves the device on the image it shows, so a passing outage does not
//! blank the display; a device with no image yet shows what went wrong. With no
//! current entry, the device shows a screen saying so.

use afterimage_trmnl::catalog::{Catalog, Model, Palette};
use serde_json::{Value, json};

use crate::devices::{Device, Devices, Showing};
use crate::drawing::{self, Drawing, Problem};
use crate::images::Images;
use crate::schedule;
use crate::screens::Screens;

/// What the check-in answers.
pub struct Shown {
    /// The image's name in [`Images`].
    pub image: String,
    /// Seconds until the device checks in again.
    pub wait: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Devices(#[from] crate::devices::Error),
    #[error("screen storage: {0}")]
    Screens(#[from] sqlx::Error),
    #[error("image storage: {0}")]
    Images(#[from] std::io::Error),
    #[error("the server's own screen could not be drawn: {0}")]
    Fallback(String),
}

/// The model and palette a device's display names; the catalogue's default for
/// its firmware when the catalogue no longer lists them.
#[must_use]
pub fn display(device: &Device) -> (&'static Model, &'static Palette) {
    let catalog = Catalog::get();
    match (
        catalog.model(&device.display.model),
        catalog.palette(&device.display.palette),
    ) {
        (Some(model), Some(palette)) => (model, palette),
        _ => catalog.default_display(&device.firmware_model, None),
    }
}

/// The device as templates read it.
#[must_use]
pub fn context(name: &str, model: &Model) -> drawing::Device {
    drawing::Device {
        name: name.to_owned(),
        width: f64::from(model.width) / model.scale_factor,
        height: f64::from(model.height) / model.scale_factor,
    }
}

/// Draws what the device shows next and records it.
///
/// # Errors
///
/// When storage fails or the server cannot draw its own screen.
pub async fn next(
    device: &Device,
    devices: &Devices,
    screens: &Screens,
    drawing: &Drawing,
    images: &Images,
    now: i64,
) -> Result<Shown, Error> {
    let at = jiff::Timestamp::from_second(now)
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
        .to_zoned(drawing.time_zone().clone());
    let wait = schedule::wait(&device.playlist, device.refresh_minutes, &at);
    let (model, palette) = display(device);
    let position = schedule::next(&device.playlist, device.playlist_position, &at);
    let drawn = if let Some(position) = position {
        let screen_id = device.playlist[position].screen_id;
        match screens.get(device.account_id, screen_id).await? {
            Some(screen) => {
                let document: Value = serde_json::from_str(&screen.document).unwrap_or(Value::Null);
                let drawn = drawing
                    .draw(
                        &document,
                        &context(&device.name, model),
                        model,
                        palette,
                        now,
                    )
                    .await;
                match (drawn.image, drawn.problems.as_slice()) {
                    (Some(image), []) => Ok((image.bytes, Some(screen_id))),
                    (_, problems) => Err((screen.name, problems.to_vec())),
                }
            }
            None => Err((String::new(), Vec::new())),
        }
    } else {
        let image = drawing
            .draw_resolved(nothing_to_show(&device.name), model, palette)
            .await
            .map_err(Error::Fallback)?;
        Ok((image.bytes, None))
    };
    let (bytes, screen_id) = match drawn {
        Ok(drawn) => drawn,
        Err((name, problems)) => {
            for problem in &problems {
                tracing::warn!(
                    device = device.id,
                    screen = name,
                    path = problem.path,
                    "{}",
                    problem.message
                );
            }
            if let Some(showing) = &device.showing {
                return Ok(Shown {
                    image: showing.image.clone(),
                    wait,
                });
            }
            let image = drawing
                .draw_resolved(could_not_draw(&name, &problems), model, palette)
                .await
                .map_err(Error::Fallback)?;
            (image.bytes, None)
        }
    };
    let keep = device
        .showing
        .as_ref()
        .map(|showing| showing.image.as_str());
    let image = images.save(device.id, &bytes, keep)?;
    let changed = device
        .showing
        .as_ref()
        .is_none_or(|showing| showing.image != image);
    let rendered_at = if changed {
        now
    } else {
        device
            .showing
            .as_ref()
            .map_or(now, |showing| showing.rendered_at)
    };
    let showing = Showing {
        image: image.clone(),
        screen_id,
        rendered_at,
    };
    devices.show(device.id, position, &showing).await?;
    Ok(Shown { image, wait })
}

/// The server's screen for a device whose playlist has nothing current.
fn nothing_to_show(device: &str) -> Value {
    json!({
        "titleBar": { "title": "Afterimage", "instance": device },
        "content": { "column": { "align": "center", "justify": "center", "gap": "medium", "grow": false, "children": [
            { "title": { "text": "Nothing to show", "size": "large" } },
            { "description": { "text": "Add a screen to this device's playlist in Afterimage." } }
        ] } }
    })
}

/// The server's screen for a device whose first screen could not be drawn.
fn could_not_draw(screen: &str, problems: &[Problem]) -> Value {
    let reasons: Vec<Value> = problems
        .iter()
        .take(4)
        .map(|problem| json!({ "description": { "text": problem.message, "clamp": 2 } }))
        .collect();
    let mut children =
        vec![json!({ "title": { "text": "This screen could not be drawn", "size": "large" } })];
    if reasons.is_empty() {
        children.push(json!({ "description": { "text": "The screen no longer exists." } }));
    }
    children.extend(reasons);
    json!({
        "titleBar": { "title": "Afterimage", "instance": screen },
        "content": { "column": { "align": "start", "justify": "center", "gap": "medium", "grow": false, "children": children } }
    })
}
