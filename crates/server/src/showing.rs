//! What a claimed device shows at a check-in: its sleep screen while it sleeps
//! or is held asleep, else its pinned screen or the next current entry of its
//! playlist, drawn now, or one of the server's own screens.
//!
//! A playlist screen that cannot be drawn as written (a data source down, a
//! template error) leaves the device on the image it shows, so a passing outage
//! does not blank the display; a device with no image yet, or one just woken
//! from its sleep screen, shows what went wrong.
//! With no current entry, the device shows a screen saying so. A sleep screen
//! that cannot be drawn gives way to the server's own, which says when the
//! device wakes, so a daytime image does not stand all night.

use afterimage_render_typst::Renderer;
use afterimage_trmnl::catalog::{Catalog, Model, Palette};
use serde_json::{Value, json};

use crate::devices::{Device, Devices, Entry, Showing, Shows};
use crate::drawing::{self, Drawing, Problem};
use crate::images::Images;
use crate::schedule;
use crate::screens::Screens;

/// Problems logged for a screen that could not be drawn; the editor's preview
/// shows them all.
const LOGGED_PROBLEMS: usize = 4;

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

/// The model and palette a device's display names; when they can no longer be
/// drawn, as after a catalogue refresh, the one a claim would choose now.
#[must_use]
pub fn display(device: &Device) -> (&'static Model, &'static Palette) {
    drawing::drawable(&device.display.model, &device.display.palette).unwrap_or_else(|| {
        Catalog::get().default_display(&device.firmware_model, device.panel, Renderer::supports)
    })
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
        .to_zoned(drawing.time_zone().zone().clone());
    let playlist: &[Entry] = match device.shows {
        Shows::Playlist => &device.playlist,
        Shows::Screen(_) => &[],
    };
    let in_window = schedule::asleep(&device.sleep.windows, &at);
    let wait = schedule::wait(
        playlist,
        &device.sleep.windows,
        device.sleep_held,
        device.refresh_minutes,
        &at,
    );
    let shown = Record {
        device,
        devices,
        images,
        now,
        wait,
    };
    if in_window || device.sleep_held {
        return asleep(&shown, screens, drawing, &at, in_window).await;
    }
    // The server's own screens share the check-in's time with the screen drawn first.
    let places_by = Drawing::places_deadline();
    let (model, palette) = display(device);
    // A pinned screen stands in for the playlist, which waits where it was.
    let (position, screen) = match device.shows {
        Shows::Screen(screen_id) => (device.playlist_position, Some(screen_id)),
        Shows::Playlist => {
            let position = schedule::next(&device.playlist, device.playlist_position, &at);
            (
                position,
                position.map(|position| device.playlist[position].screen_id),
            )
        }
    };
    let drawn = if let Some(screen_id) = screen {
        drawn_screen(device, screens, drawing, screen_id, now)
            .await?
            .map(|bytes| (bytes, Some(screen_id)))
    } else {
        let image = drawing
            .draw_resolved(nothing_to_show(&device.name), model, palette, places_by)
            .await
            .map_err(Error::Fallback)?;
        Ok((image.bytes, None))
    };
    let (bytes, screen_id) = match drawn {
        Ok(drawn) => drawn,
        Err(Failed { name, problems }) => {
            log_problems(device.id, &name, &problems);
            // A sleep image is not kept: the device is awake, and says what went
            // wrong rather than that it sleeps.
            if let Some(showing) = device
                .showing
                .as_ref()
                .filter(|showing| !showing.asleep && images.has(device.id, &showing.image))
            {
                // The device keeps its image, and the playlist moves past the entry
                // that failed, so the next check-in shows the entry after it.
                devices
                    .show(device.id, position, showing, now + wait)
                    .await?;
                return Ok(Shown {
                    image: showing.image.clone(),
                    wait,
                });
            }
            let image = drawing
                .draw_resolved(could_not_draw(&name, &problems), model, palette, places_by)
                .await
                .map_err(Error::Fallback)?;
            (image.bytes, None)
        }
    };
    shown.record(&bytes, screen_id, position, false).await
}

/// The device's sleep screen, or the server's own when it has none or it
/// cannot be drawn, which says when a sleep window ends. The playlist stays
/// where it was.
async fn asleep(
    shown: &Record<'_>,
    screens: &Screens,
    drawing: &Drawing,
    at: &jiff::Zoned,
    in_window: bool,
) -> Result<Shown, Error> {
    let device = shown.device;
    // The server's own screen shares the check-in's time with the screen drawn first.
    let places_by = Drawing::places_deadline();
    if let Some(screen_id) = device.sleep.screen_id {
        match drawn_screen(device, screens, drawing, screen_id, shown.now).await? {
            Ok(bytes) => {
                return shown
                    .record(&bytes, Some(screen_id), device.playlist_position, true)
                    .await;
            }
            Err(Failed { name, problems }) => log_problems(device.id, &name, &problems),
        }
    }
    let (model, palette) = display(device);
    // Held asleep outside its windows, the device does not know when it wakes.
    let wakes = in_window.then(|| {
        at.checked_add(jiff::SignedDuration::from_secs(shown.wait))
            .unwrap_or_else(|_| at.clone())
    });
    let image = drawing
        .draw_resolved(
            sleeping(&device.name, at, wakes.as_ref()),
            model,
            palette,
            places_by,
        )
        .await
        .map_err(Error::Fallback)?;
    shown
        .record(&image.bytes, None, device.playlist_position, true)
        .await
}

/// A screen that could not be drawn as written, and why.
struct Failed {
    name: String,
    problems: Vec<Problem>,
}

/// Draws one of the account's screens for the device; `Err` names what failed.
async fn drawn_screen(
    device: &Device,
    screens: &Screens,
    drawing: &Drawing,
    screen_id: i64,
    now: i64,
) -> Result<Result<Vec<u8>, Failed>, Error> {
    let Some(screen) = screens.get(device.account_id, screen_id).await? else {
        return Ok(Err(Failed {
            name: String::new(),
            problems: Vec::new(),
        }));
    };
    let (model, palette) = display(device);
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
    Ok(match (drawn.image, drawn.problems.as_slice()) {
        (Some(image), []) => Ok(image.bytes),
        (_, problems) => Err(Failed {
            name: screen.name,
            problems: problems.to_vec(),
        }),
    })
}

fn log_problems(device: i64, screen: &str, problems: &[Problem]) {
    for problem in problems.iter().take(LOGGED_PROBLEMS) {
        tracing::warn!(device, screen, path = problem.path, "{}", problem.message);
    }
}

/// The check-in being answered, which records what the device shows.
struct Record<'a> {
    device: &'a Device,
    devices: &'a Devices,
    images: &'a Images,
    now: i64,
    /// Seconds until the device checks in again.
    wait: i64,
}

impl Record<'_> {
    /// Saves the image and records it as what the device shows until it wakes.
    async fn record(
        &self,
        bytes: &[u8],
        screen_id: Option<i64>,
        position: Option<usize>,
        asleep: bool,
    ) -> Result<Shown, Error> {
        let device = self.device;
        let image = self.images.save(device.id, bytes)?;
        let rendered_at = device
            .showing
            .as_ref()
            .filter(|showing| showing.image == image)
            .map_or(self.now, |showing| showing.rendered_at);
        let showing = Showing {
            image: image.clone(),
            screen_id,
            rendered_at,
            asleep,
        };
        self.devices
            .show(device.id, position, &showing, self.now + self.wait)
            .await?;
        Ok(Shown {
            image,
            wait: self.wait,
        })
    }
}

/// The server's screen for a sleeping device: that it sleeps, and when it wakes
/// where that is known.
fn sleeping(device: &str, at: &jiff::Zoned, wakes: Option<&jiff::Zoned>) -> Value {
    let mut children = vec![json!({ "title": { "text": "Asleep", "size": "large" } })];
    if let Some(wakes) = wakes {
        let time = wakes.strftime("%H:%M").to_string();
        let when = if wakes.date() == at.date() {
            format!("Wakes at {time}")
        } else {
            format!("Wakes tomorrow at {time}")
        };
        children.push(json!({ "description": { "text": when } }));
    }
    json!({
        "titleBar": { "title": "Afterimage", "instance": device },
        "content": { "column": { "align": "center", "justify": "center", "gap": "medium", "grow": false, "children": children } }
    })
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
