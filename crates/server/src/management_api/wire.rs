//! Conversions between what the stores hold and the model's shapes.
//!
//! Stored values were validated when they were written, so a value that no longer
//! fits its shape is a server fault, answered as `InternalFailure`.

use afterimage_management_api::error::InternalFailure;
use afterimage_management_api::model;
use afterimage_management_api::serde::{SerializationSettings, SerializeConfigured};
use afterimage_trmnl::catalog::Catalog;
use aws_smithy_types::DateTime;

use crate::accounts::Account;
use crate::devices::{self, Device, Entry, Hours, Shows, Sleep};
use crate::drawing;
use crate::firmware::{self, Board, Firmware};
use crate::screens::{Screen, Summary};

/// A stored value into its constrained model shape.
pub fn fit<T, V>(value: V) -> Result<T, InternalFailure>
where
    T: TryFrom<V>,
    T::Error: std::fmt::Debug,
{
    T::try_from(value).map_err(|violation| {
        super::internal(&format!(
            "a stored value breaks its model constraint: {violation:?}"
        ))
    })
}

pub fn account(account: Account) -> Result<model::Account, InternalFailure> {
    Ok(model::Account {
        id: fit(account.id)?,
        name: fit(account.name)?,
        username: account.username.map(fit).transpose()?,
        administrator: account.administrator,
    })
}

/// A device, with its board and that board's newest release from TRMNL's
/// `boards` when they could be read.
pub fn device(device: Device, boards: Option<&[Board]>) -> Result<model::Device, InternalFailure> {
    let overdue_at = device.overdue_at();
    let board = boards.and_then(|boards| Firmware::board(boards, &device));
    let firmware = model::DeviceFirmware {
        board: device
            .firmware_board
            .clone()
            .or_else(|| board.map(|board| board.name.clone()))
            .map(fit)
            .transpose()?,
        board_chosen: device.firmware_board.is_some(),
        latest_version: board
            .and_then(Board::latest)
            .map(|release| fit(release.version.clone()))
            .transpose()?,
        update_available: board
            .is_some_and(|board| board.newer_than(device.readings.firmware_version.as_deref())),
        update: device.firmware_update.map(firmware_update).transpose()?,
    };
    Ok(model::Device {
        id: fit(device.id)?,
        name: fit(device.name)?,
        mac: fit(device.mac)?,
        firmware_model: device.firmware_model,
        display: model::Display {
            model: fit(device.display.model)?,
            palette: fit(device.display.palette)?,
        },
        refresh_minutes: fit(i32::try_from(device.refresh_minutes).unwrap_or(i32::MAX))?,
        playlist: fit(device
            .playlist
            .into_iter()
            .map(entry)
            .collect::<Result<Vec<_>, _>>()?)?,
        shows: shows(device.shows)?,
        sleep: sleep(device.sleep)?,
        sleep_held: device.sleep_held,
        readings: model::Readings {
            battery_voltage: device.readings.battery_voltage,
            battery_percent: device.readings.battery_percent,
            signal: device.readings.signal_dbm,
            firmware_version: device.readings.firmware_version,
        },
        seen_at: device.seen_at.map(DateTime::from_secs),
        wakes_at: device.wakes_at.map(DateTime::from_secs),
        overdue_at: overdue_at.map(DateTime::from_secs),
        showing: device
            .showing
            .map(|showing| -> Result<_, InternalFailure> {
                Ok(model::Showing {
                    screen_id: showing.screen_id.map(fit).transpose()?,
                    rendered_at: DateTime::from_secs(showing.rendered_at),
                    asleep: showing.asleep,
                })
            })
            .transpose()?,
        firmware,
    })
}

fn firmware_update(update: firmware::Update) -> Result<model::FirmwareUpdate, InternalFailure> {
    Ok(model::FirmwareUpdate {
        state: match update.state() {
            firmware::State::Requested => model::FirmwareUpdateState::Requested,
            firmware::State::Sent => model::FirmwareUpdateState::Sent,
            firmware::State::Installed => model::FirmwareUpdateState::Installed,
            firmware::State::NotInstalled => model::FirmwareUpdateState::NotInstalled,
        },
        version: fit(update.version)?,
        requested_at: DateTime::from_secs(update.requested_at),
        sent_at: update.sent_at.map(DateTime::from_secs),
        finished_at: update.finished_at.map(DateTime::from_secs),
    })
}

pub fn display(display: model::Display) -> devices::Display {
    devices::Display {
        model: display.model.into_inner(),
        palette: display.palette.into_inner(),
    }
}

/// Weekdays in the stored bit order, Monday as bit 0.
const WEEK: [model::Weekday; 7] = [
    model::Weekday::Monday,
    model::Weekday::Tuesday,
    model::Weekday::Wednesday,
    model::Weekday::Thursday,
    model::Weekday::Friday,
    model::Weekday::Saturday,
    model::Weekday::Sunday,
];

/// Hours in the model's terms: days by name, times as `HH:MM`.
struct WireHours {
    days: Option<model::Weekdays>,
    from: Option<model::TimeOfDay>,
    until: Option<model::TimeOfDay>,
}

fn wire_hours(hours: Hours) -> Result<WireHours, InternalFailure> {
    let days = hours
        .days
        .map(|bits| {
            WEEK.iter()
                .enumerate()
                .filter(|(bit, _)| bits & (1 << bit) != 0)
                .map(|(_, day)| *day)
                .collect::<Vec<_>>()
        })
        .map(fit)
        .transpose()?;
    let time = |minute: Option<u16>| {
        minute
            .map(|minute| fit(format!("{:02}:{:02}", minute / 60, minute % 60)))
            .transpose()
    };
    Ok(WireHours {
        days,
        from: time(hours.from_minute)?,
        until: time(hours.until_minute)?,
    })
}

/// Hours as the store keeps them. The model's patterns guarantee the times parse.
fn stored_hours(
    days: Option<&model::Weekdays>,
    from: Option<&model::TimeOfDay>,
    until: Option<&model::TimeOfDay>,
) -> Hours {
    let minute = |time: Option<&model::TimeOfDay>| {
        time.and_then(|time| {
            let (hours, minutes) = time.as_str().split_once(':')?;
            Some(hours.parse::<u16>().ok()? * 60 + minutes.parse::<u16>().ok()?)
        })
    };
    Hours {
        days: days.map(|days| {
            days.inner().iter().fold(0, |bits, day| {
                let bit = WEEK.iter().position(|week| week == day).unwrap_or(0);
                bits | (1 << bit)
            })
        }),
        from_minute: minute(from),
        until_minute: minute(until),
    }
}

fn entry(entry: Entry) -> Result<model::PlaylistEntry, InternalFailure> {
    let WireHours { days, from, until } = wire_hours(entry.hours)?;
    Ok(model::PlaylistEntry {
        screen_id: fit(entry.screen_id)?,
        days,
        from,
        until,
    })
}

/// A playlist entry as the store keeps it.
pub fn stored_entry(entry: &model::PlaylistEntry) -> Entry {
    Entry {
        screen_id: *entry.screen_id.inner(),
        hours: stored_hours(
            entry.days.as_ref(),
            entry.from.as_ref(),
            entry.until.as_ref(),
        ),
    }
}

fn shows(shows: Shows) -> Result<model::Shows, InternalFailure> {
    Ok(match shows {
        Shows::Playlist => model::Shows::Playlist,
        Shows::Screen(id) => model::Shows::ScreenId(fit(id)?),
    })
}

/// What an awake device shows, as the store keeps it.
pub fn stored_shows(shows: &model::Shows) -> Shows {
    match shows {
        model::Shows::Playlist => Shows::Playlist,
        model::Shows::ScreenId(id) => Shows::Screen(*id.inner()),
    }
}

fn sleep(sleep: Sleep) -> Result<model::Sleep, InternalFailure> {
    let windows = sleep
        .windows
        .into_iter()
        .map(|window| {
            let WireHours { days, from, until } = wire_hours(window)?;
            Ok(model::SleepWindow { days, from, until })
        })
        .collect::<Result<Vec<_>, InternalFailure>>()?;
    Ok(model::Sleep {
        windows: fit(windows)?,
        screen_id: sleep.screen_id.map(fit).transpose()?,
    })
}

/// A device's sleep as the store keeps it.
pub fn stored_sleep(sleep: &model::Sleep) -> Sleep {
    Sleep {
        windows: sleep
            .windows
            .inner()
            .iter()
            .map(|window| {
                stored_hours(
                    window.days.as_ref(),
                    window.from.as_ref(),
                    window.until.as_ref(),
                )
            })
            .collect(),
        screen_id: sleep.screen_id.as_ref().map(|id| *id.inner()),
    }
}

/// The displays Afterimage can draw for: each catalogue model with the palettes the
/// renderer can draw it in, and none of the models it cannot draw at all.
pub fn display_models() -> Result<Vec<model::DisplayModel>, InternalFailure> {
    let mut models = Vec::new();
    for display in &Catalog::get().models {
        let palettes = display
            .palette_ids
            .iter()
            .filter_map(|id| drawing::drawable(&display.name, id).map(|(_, palette)| palette))
            .map(|palette| {
                Ok(model::Palette {
                    id: fit(palette.id.clone())?,
                    name: palette.name.clone(),
                    colors: palette
                        .rgb()
                        .map(|colors| {
                            colors
                                .iter()
                                .map(|[r, g, b]| fit(format!("#{r:02x}{g:02x}{b:02x}")))
                                .collect::<Result<_, InternalFailure>>()
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, InternalFailure>>()?;
        if palettes.is_empty() {
            continue;
        }
        models.push(model::DisplayModel {
            name: fit(display.name.clone())?,
            label: display.label.clone(),
            width: i32::try_from(display.width).unwrap_or(i32::MAX),
            height: i32::try_from(display.height).unwrap_or(i32::MAX),
            palettes,
        });
    }
    Ok(models)
}

/// A document as the screens table stores it: the model's serde form, as JSON.
pub fn stored_document(document: &model::ScreenDocument) -> Result<String, InternalFailure> {
    let settings = SerializationSettings::leak_sensitive_fields();
    serde_json::to_string(&document.serialize_ref(&settings))
        .map_err(|error| super::internal(&error))
}

pub fn document(stored: &str) -> Result<model::ScreenDocument, InternalFailure> {
    serde_json::from_str(stored).map_err(|error| super::internal(&error))
}

pub fn screen(screen: Screen) -> Result<model::Screen, InternalFailure> {
    Ok(model::Screen {
        id: fit(screen.id)?,
        name: fit(screen.name)?,
        document: document(&screen.document)?,
        updated_at: DateTime::from_secs(screen.updated_at),
    })
}

pub fn summary(screen: Summary) -> Result<model::ScreenSummary, InternalFailure> {
    Ok(model::ScreenSummary {
        id: fit(screen.id)?,
        name: fit(screen.name)?,
        updated_at: DateTime::from_secs(screen.updated_at),
    })
}

/// JSON as a Smithy document.
pub fn json(value: serde_json::Value) -> aws_smithy_types::Document {
    use aws_smithy_types::{Document, Number};
    match value {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(value) => Document::Bool(value),
        serde_json::Value::Number(number) => Document::Number(
            number
                .as_u64()
                .map(Number::PosInt)
                .or_else(|| number.as_i64().map(Number::NegInt))
                .unwrap_or_else(|| Number::Float(number.as_f64().unwrap_or(0.0))),
        ),
        serde_json::Value::String(text) => Document::String(text),
        serde_json::Value::Array(items) => Document::Array(items.into_iter().map(json).collect()),
        serde_json::Value::Object(members) => Document::Object(
            members
                .into_iter()
                .map(|(name, member)| (name, json(member)))
                .collect(),
        ),
    }
}
