//! Conversions between what the stores hold and the model's shapes.
//!
//! Stored values were validated when they were written, so a value that no longer
//! fits its shape is a server fault, answered as `InternalFailure`.

use aws_smithy_types::DateTime;
use afterimage_management_api::error::InternalFailure;
use afterimage_management_api::model;
use afterimage_management_api::serde::{SerializationSettings, SerializeConfigured};
use afterimage_trmnl::catalog::Catalog;

use crate::accounts::Account;
use crate::devices::{self, Device, Entry};
use crate::screens::{Screen, Summary};

/// A stored value into its constrained model shape.
fn fit<T, V>(value: V) -> Result<T, InternalFailure>
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
    })
}

pub fn device(device: Device) -> Result<model::Device, InternalFailure> {
    Ok(model::Device {
        id: fit(device.id)?,
        name: fit(device.name)?,
        mac: device.mac,
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
        readings: model::Readings {
            battery_voltage: device.readings.battery_voltage,
            battery_percent: device.readings.battery_percent,
            signal: device.readings.signal_dbm,
            firmware_version: device.readings.firmware_version,
        },
        seen_at: device.seen_at.map(DateTime::from_secs),
        showing: device
            .showing
            .map(|showing| -> Result<_, InternalFailure> {
                Ok(model::Showing {
                    screen_id: showing.screen_id.map(fit).transpose()?,
                    rendered_at: DateTime::from_secs(showing.rendered_at),
                })
            })
            .transpose()?,
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

fn entry(entry: Entry) -> Result<model::PlaylistEntry, InternalFailure> {
    let days = entry
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
    Ok(model::PlaylistEntry {
        screen_id: fit(entry.screen_id)?,
        days,
        from: time(entry.from_minute)?,
        until: time(entry.until_minute)?,
    })
}

/// A playlist entry as the store keeps it. The model's patterns guarantee the
/// times parse.
pub fn stored_entry(entry: &model::PlaylistEntry) -> Entry {
    let minute = |time: &Option<model::TimeOfDay>| {
        time.as_ref().and_then(|time| {
            let (hours, minutes) = time.as_str().split_once(':')?;
            Some(hours.parse::<u16>().ok()? * 60 + minutes.parse::<u16>().ok()?)
        })
    };
    Entry {
        screen_id: *entry.screen_id.inner(),
        days: entry.days.as_ref().map(|days| {
            days.inner().iter().fold(0, |bits, day| {
                let bit = WEEK.iter().position(|week| week == day).unwrap_or(0);
                bits | (1 << bit)
            })
        }),
        from_minute: minute(&entry.from),
        until_minute: minute(&entry.until),
    }
}

pub fn display_models() -> Result<Vec<model::DisplayModel>, InternalFailure> {
    let catalog = Catalog::get();
    catalog
        .models
        .iter()
        .map(|display| {
            Ok(model::DisplayModel {
                name: fit(display.name.clone())?,
                label: display.label.clone(),
                width: i32::try_from(display.width).unwrap_or(i32::MAX),
                height: i32::try_from(display.height).unwrap_or(i32::MAX),
                palettes: display
                    .palette_ids
                    .iter()
                    .filter_map(|id| catalog.palette(id))
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
                    .collect::<Result<_, InternalFailure>>()?,
            })
        })
        .collect()
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
