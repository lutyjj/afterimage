//! What the server publishes about a device: the key that names it in topics,
//! its state and firmware as Home Assistant's entities read them, the names its
//! screens take as options, and its discovery payload.

use std::str::FromStr;

use afterimage_trmnl::Mac;
use serde::Serialize;
use serde_json::{Value, json};

use super::Settings;
use crate::devices::{Device, Shows};
use crate::firmware::{self, Board, Firmware};
use crate::screens::Summary;

/// The option of the "Showing" select that follows the playlist.
pub(super) const PLAYLIST: &str = "Playlist";

/// The "Wakes every" number's bounds, which the model's `RefreshMinutes` sets
/// (a test holds them to it).
const FEWEST_MINUTES: i32 = 1;
const MOST_MINUTES: i32 = 1440;

/// The account's screens as the "Showing" select names them, after its
/// playlist: each by its name, a name two screens share numbered by age, so an
/// option names one screen.
pub(super) fn screen_options(screens: &[Summary]) -> Vec<(String, i64)> {
    let mut sorted: Vec<&Summary> = screens.iter().collect();
    sorted.sort_by_key(|screen| screen.id);
    let mut options: Vec<(String, i64)> = Vec::new();
    for screen in sorted {
        let mut name = screen.name.clone();
        let mut count = 1;
        while name == PLAYLIST || options.iter().any(|(taken, _)| *taken == name) {
            count += 1;
            name = format!("{} ({count})", screen.name);
        }
        options.push((name, screen.id));
    }
    options
}

/// A device's key in topics: its MAC in lowercase hexadecimal.
pub(super) fn key(mac: &str) -> Option<String> {
    let mac = Mac::from_str(mac).ok()?.to_string();
    Some(mac.replace(':', "").to_lowercase())
}

/// The MAC, as the registrations store it, of a key from a topic.
pub(super) fn mac_of(key: &str) -> Option<String> {
    if key.len() != 12 {
        return None;
    }
    let pairs: Vec<&str> = (0..6)
        .filter_map(|index| key.get(index * 2..index * 2 + 2))
        .collect();
    Mac::from_str(&pairs.join(":"))
        .ok()
        .map(|mac| mac.to_string())
}

/// The device key of a topic `<base>/<key>/<leaf>`.
pub(super) fn device_key<'a>(base: &str, topic: &'a str, leaf: &str) -> Option<&'a str> {
    topic
        .strip_prefix(base)?
        .strip_prefix('/')?
        .strip_suffix(leaf)?
        .strip_suffix('/')
        .filter(|key| !key.is_empty() && !key.contains('/'))
}

/// An instant as Home Assistant's timestamp sensors read it.
fn timestamp(seconds: Option<i64>) -> Option<String> {
    seconds
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|time| time.to_string())
}

/// What the device's entities read: its readings, the screen it shows, whether
/// it sleeps and is held asleep, when it wakes, whether it counts as not seen
/// lately at `now`, how often it wakes, and what it shows while awake as the
/// "Showing" option (`options` from [`screen_options`]).
pub(super) fn state(device: &Device, screen: &str, options: &[(String, i64)], now: i64) -> Value {
    let showing = match device.shows {
        Shows::Playlist => PLAYLIST.to_owned(),
        Shows::Screen(id) => options
            .iter()
            .find(|(_, option)| *option == id)
            .map_or_else(|| PLAYLIST.to_owned(), |(name, _)| name.clone()),
    };
    json!({
        "battery": device.readings.battery_percent.map(f64::round),
        "voltage": device.readings.battery_voltage,
        "signal": device.readings.signal_dbm,
        "seen_at": timestamp(device.seen_at),
        "screen": screen,
        "asleep": device.showing.as_ref().is_some_and(|showing| showing.asleep),
        "held": device.sleep_held,
        "wakes_at": timestamp(device.wakes_at),
        "overdue": overdue(device, now),
        "refresh": device.refresh_minutes,
        "showing": showing,
    })
}

/// Whether the device counts as not seen lately at `now`.
pub(super) fn overdue(device: &Device, now: i64) -> bool {
    device.overdue_at().is_some_and(|at| now > at)
}

/// The state of Home Assistant's update entity. A version the server does not
/// know is left out: the entity refuses a null one, which it reads as a string.
#[derive(Serialize)]
struct FirmwareState<'a> {
    in_progress: bool,
    title: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_version: Option<&'a str>,
}

/// The device's firmware, in the shape of Home Assistant's update entity. An
/// install request between its request and the check-in that decides it is in
/// progress.
pub(super) fn firmware_state(device: &Device, boards: Option<&[Board]>) -> Value {
    let latest = boards
        .and_then(|boards| Firmware::board(boards, device))
        .and_then(Board::latest);
    json!(FirmwareState {
        in_progress: device.firmware_update.as_ref().is_some_and(|update| {
            matches!(
                update.state(),
                firmware::State::Requested | firmware::State::Sent
            )
        }),
        title: "TRMNL firmware",
        installed_version: device.readings.firmware_version.as_deref(),
        latest_version: latest.map(|release| release.version.as_str()),
    })
}

/// The device as Home Assistant's discovery describes it. Home Assistant drops
/// a device whose optional fields are null, so an unknown one is left out.
#[derive(Serialize)]
struct DeviceInfo<'a> {
    identifiers: [String; 1],
    connections: [(&'static str, String); 1],
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sw_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration_url: Option<String>,
}

/// The device's discovery payload: the device, the server as its origin, and
/// one component per entity.
pub(super) fn discovery(
    settings: &Settings,
    key: &str,
    device: &Device,
    options: &[(String, i64)],
) -> Value {
    let base = &settings.topic;
    let id = |entity: &str| format!("{base}_{key}_{entity}");
    let state_topic = format!("{base}/{key}/state");
    let sensor = |name: &str, field: &str, extra: Value| {
        let mut component = json!({
            "platform": "sensor",
            "name": name,
            "unique_id": id(field),
            "state_topic": state_topic,
            "value_template": format!("{{{{ value_json.{field} }}}}"),
        });
        if let (Some(component), Value::Object(extra)) = (component.as_object_mut(), extra) {
            component.extend(extra);
        }
        component
    };
    let device_info = DeviceInfo {
        identifiers: [format!("{base}_{key}")],
        connections: [("mac", device.mac.to_lowercase())],
        name: &device.name,
        model: Some(device.firmware_model.as_str()).filter(|model| !model.is_empty()),
        sw_version: device.readings.firmware_version.as_deref(),
        configuration_url: settings
            .public_url
            .as_ref()
            .map(|url| format!("{url}/devices/{}", device.id)),
    };
    let mut payload = json!({
        "device": device_info,
        "origin": { "name": "Afterimage", "sw_version": env!("CARGO_PKG_VERSION") },
        "availability_topic": format!("{base}/status"),
        "components": {
            "battery": sensor("Battery", "battery", json!({
                "device_class": "battery",
                "unit_of_measurement": "%",
                "state_class": "measurement",
                "entity_category": "diagnostic",
            })),
            "voltage": sensor("Battery voltage", "voltage", json!({
                "device_class": "voltage",
                "unit_of_measurement": "V",
                "state_class": "measurement",
                "entity_category": "diagnostic",
            })),
            "signal": sensor("Signal", "signal", json!({
                "device_class": "signal_strength",
                "unit_of_measurement": "dBm",
                "state_class": "measurement",
                "entity_category": "diagnostic",
            })),
            "seen_at": sensor("Last check-in", "seen_at", json!({
                "device_class": "timestamp",
                "entity_category": "diagnostic",
            })),
            "screen": sensor("Screen", "screen", json!({ "icon": "mdi:image-frame" })),
            "wakes_at": sensor("Wakes at", "wakes_at", json!({
                "device_class": "timestamp",
                "entity_category": "diagnostic",
            })),
            "asleep": {
                "platform": "binary_sensor",
                "name": "Asleep",
                "unique_id": id("asleep"),
                "icon": "mdi:sleep",
                "state_topic": state_topic,
                "value_template": "{{ 'ON' if value_json.asleep else 'OFF' }}",
            },
            "overdue": {
                "platform": "binary_sensor",
                "name": "Not seen lately",
                "unique_id": id("overdue"),
                "device_class": "problem",
                "state_topic": state_topic,
                "value_template": "{{ 'ON' if value_json.overdue else 'OFF' }}",
            },
            "firmware": {
                "platform": "update",
                "name": "Firmware",
                "unique_id": id("firmware"),
                "device_class": "firmware",
                "entity_category": "config",
                "state_topic": format!("{base}/{key}/firmware"),
                "command_topic": format!("{base}/{key}/firmware/install"),
                "payload_install": "install",
            },
        },
    });
    if let Some(components) = payload["components"].as_object_mut() {
        components.extend(controls(base, key, options));
    }
    payload
}

/// The device's controls in Home Assistant: a switch that holds it asleep, a
/// select of what it shows while awake, how often it wakes, its button's
/// presses, and the image it shows.
fn controls(base: &str, key: &str, options: &[(String, i64)]) -> serde_json::Map<String, Value> {
    let id = |entity: &str| format!("{base}_{key}_{entity}");
    let state_topic = format!("{base}/{key}/state");
    json!({
        "held": {
            "platform": "switch",
            "name": "Sleep",
            "unique_id": id("held"),
            "icon": "mdi:sleep",
            "state_topic": state_topic,
            "value_template": "{{ 'ON' if value_json.held else 'OFF' }}",
            "command_topic": format!("{base}/{key}/held/set"),
        },
        "showing": {
            "platform": "select",
            "name": "Showing",
            "unique_id": id("showing"),
            "icon": "mdi:pin",
            "state_topic": state_topic,
            "value_template": "{{ value_json.showing }}",
            "command_topic": format!("{base}/{key}/showing/set"),
            "options": std::iter::once(PLAYLIST.to_owned())
                .chain(options.iter().map(|(name, _)| name.clone()))
                .collect::<Vec<_>>(),
        },
        "refresh": {
            "platform": "number",
            "name": "Wakes every",
            "unique_id": id("refresh"),
            "entity_category": "config",
            "unit_of_measurement": "min",
            "min": FEWEST_MINUTES,
            "max": MOST_MINUTES,
            "step": 1,
            "mode": "box",
            "state_topic": state_topic,
            "value_template": "{{ value_json.refresh }}",
            "command_topic": format!("{base}/{key}/refresh/set"),
        },
        "button": {
            "platform": "event",
            "name": "Button",
            "unique_id": id("button"),
            "device_class": "button",
            "event_types": ["press"],
            "state_topic": format!("{base}/{key}/button"),
        },
        "image": {
            "platform": "image",
            "name": "Image",
            "unique_id": id("image"),
            "image_topic": format!("{base}/{key}/image"),
            "content_type": "image/png",
        },
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wake_bounds_are_the_models() {
        use afterimage_management_api::model::RefreshMinutes;
        assert!(RefreshMinutes::try_from(FEWEST_MINUTES).is_ok());
        assert!(RefreshMinutes::try_from(FEWEST_MINUTES - 1).is_err());
        assert!(RefreshMinutes::try_from(MOST_MINUTES).is_ok());
        assert!(RefreshMinutes::try_from(MOST_MINUTES + 1).is_err());
    }

    #[test]
    fn every_screen_option_names_one_screen() {
        let screen = |id: i64, name: &str| Summary {
            id,
            name: name.to_owned(),
            updated_at: 0,
        };
        let options = screen_options(&[
            screen(3, "Weather"),
            screen(1, "Weather"),
            screen(2, "Playlist"),
            screen(4, "Weather (2)"),
        ]);
        assert_eq!(
            options,
            [
                ("Weather".to_owned(), 1),
                ("Playlist (2)".to_owned(), 2),
                ("Weather (2)".to_owned(), 3),
                ("Weather (2) (2)".to_owned(), 4),
            ]
        );
    }

    #[test]
    fn a_key_is_the_mac_in_lowercase_hex_and_back() {
        assert_eq!(key("41:B4:10:39:A1:0F").as_deref(), Some("41b41039a10f"));
        assert_eq!(mac_of("41b41039a10f").as_deref(), Some("41:B4:10:39:A1:0F"));
        assert_eq!(mac_of("41b41039a1"), None);
        assert_eq!(mac_of("41b41039a1zz"), None);
    }

    #[test]
    fn reads_the_device_of_a_topic() {
        let install = "firmware/install";
        assert_eq!(
            device_key(
                "afterimage",
                "afterimage/41b41039a10f/firmware/install",
                install
            ),
            Some("41b41039a10f")
        );
        assert_eq!(
            device_key("afterimage", "afterimage/a/b/firmware/install", install),
            None
        );
        assert_eq!(
            device_key("afterimage", "other/41b41039a10f/firmware/install", install),
            None
        );
        assert_eq!(
            device_key("afterimage", "afterimage/41b41039a10f/state", "state"),
            Some("41b41039a10f")
        );
        assert_eq!(device_key("afterimage", "afterimage//state", "state"), None);
        assert_eq!(
            device_key("afterimage", "afterimage/a/firmware", "state"),
            None
        );
    }
}
