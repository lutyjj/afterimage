//! What the server publishes about a device: the key that names it in topics,
//! its state and firmware as Home Assistant's sensors and update entity read
//! them, and its discovery payload.

use std::str::FromStr;

use afterimage_trmnl::Mac;
use serde::Serialize;
use serde_json::{Value, json};

use super::Settings;
use crate::devices::Device;
use crate::firmware::{self, Board, Firmware};

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

/// The device's readings and screen, as its sensors read them.
pub(super) fn state(device: &Device, screen: &str) -> Value {
    let seen_at = device
        .seen_at
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|time| time.to_string());
    json!({
        "battery": device.readings.battery_percent.map(f64::round),
        "voltage": device.readings.battery_voltage,
        "signal": device.readings.signal_dbm,
        "seen_at": seen_at,
        "screen": screen,
    })
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
pub(super) fn discovery(settings: &Settings, key: &str, device: &Device) -> Value {
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
    json!({
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
