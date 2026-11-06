//! Device state over MQTT as Home Assistant reads it: discovery payloads, the
//! readings each check-in brings, availability, the firmware update entity and
//! its install command, against a Mosquitto broker of the test's own.

mod common;

use std::sync::Arc;

use common::{
    Broker, Observer, READINGS, Readings, Server, app_image, flash_image, mac, start_with,
};
use afterimage_server::keys::OsEntropy;
use reqwest::StatusCode;
use serde_json::{Value, json};

const CONFIG: &str = "homeassistant/device/afterimage_41b41039a101/config";
const STATE: &str = "afterimage/41b41039a101/state";
const FIRMWARE: &str = "afterimage/41b41039a101/firmware";

async fn start(broker: &Broker) -> Server {
    let url = broker.url();
    start_with(
        Arc::new(OsEntropy),
        &[
            "--mqtt-url",
            &url,
            "--public-url",
            "https://afterimage.example",
        ],
    )
    .await
}

#[tokio::test]
async fn a_claimed_device_is_a_home_assistant_device() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker).await;
    let server = start(&broker).await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();

    assert_eq!(observer.wait("afterimage/status", |_| true).await, "online");
    let config = observer.wait(CONFIG, |_| true).await;
    assert_eq!(
        config["device"],
        json!({
            "identifiers": ["afterimage_41b41039a101"],
            "connections": [["mac", "41:b4:10:39:a1:01"]],
            "name": device["name"],
            "model": "og",
            "sw_version": "1.8.18",
            "configuration_url": format!("https://afterimage.example/devices/{id}"),
        })
    );
    assert_eq!(config["availability_topic"], "afterimage/status");
    let components = config["components"].as_object().unwrap();
    assert_eq!(
        components.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "battery", "firmware", "screen", "seen_at", "signal", "voltage"
        ]
    );
    assert_eq!(components["battery"]["state_topic"], STATE);
    assert_eq!(components["battery"]["device_class"], "battery");
    assert_eq!(
        components["battery"]["unique_id"],
        "afterimage_41b41039a101_battery"
    );
    assert_eq!(components["firmware"]["platform"], "update");
    assert_eq!(components["firmware"]["state_topic"], FIRMWARE);
    assert_eq!(
        components["firmware"]["command_topic"],
        "afterimage/41b41039a101/firmware/install"
    );

    // The claim's check-in reported 69.4% at 4.12 V and -61 dBm.
    let state = observer.wait(STATE, |_| true).await;
    assert_eq!(state["battery"], 69.0);
    assert_eq!(state["voltage"], 4.12);
    assert_eq!(state["signal"], -61);
    assert_eq!(state["seen_at"], "2027-01-15T08:00:00Z");
    assert_eq!(state["screen"], "Nothing yet");
    let firmware = observer.wait(FIRMWARE, |_| true).await;
    assert_eq!(firmware["installed_version"], "1.8.18");
    assert_eq!(firmware["in_progress"], false);

    server.clock.advance(900);
    let weaker = Readings {
        voltage: "3.9",
        percent: "",
        rssi: "-75",
    };
    server.display(&key, &weaker).await;
    let state = observer.wait(STATE, |state| state["signal"] == -75).await;
    assert_eq!(state["battery"], 75.0, "estimated from 3.9 V");
    assert_eq!(state["seen_at"], "2027-01-15T08:15:00Z");
    assert_eq!(state["screen"], "Afterimage's own screen");
}

#[tokio::test]
async fn home_assistant_starting_again_gets_every_device_again() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker).await;
    let server = start(&broker).await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    observer.wait(CONFIG, |_| true).await;
    observer.publish(CONFIG, "", true).await;
    observer.gone(CONFIG).await;
    observer
        .publish("homeassistant/status", "online", false)
        .await;
    observer.wait(CONFIG, |_| true).await;
}

#[tokio::test]
async fn a_removed_device_leaves_home_assistant() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker).await;
    let server = start(&broker).await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    observer.wait(CONFIG, |_| true).await;
    observer.wait(STATE, |_| true).await;
    let (status, _) = server
        .call("DELETE", &format!("/manage/devices/{}", device["id"]), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    observer.gone(CONFIG).await;
    observer.gone(STATE).await;
    observer.gone(FIRMWARE).await;
}

/// Home Assistant's update entity installs by publishing `install`, which asks
/// the device for its board's newest release as the web UI does.
#[tokio::test]
async fn home_assistant_installs_firmware_through_the_update_entity() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker).await;
    let server = start(&broker).await;
    let app = app_image(b"firmware 1.8.18");
    let url = server.trmnl.publish("og-1.8.18.bin", flash_image(&app));
    server.trmnl.set_list(Some(json!({ "data": { "models": [
        { "keyname": "trmnl", "chipFamily": "ESP32-C3", "label": "TRMNL OG (native)",
          "versions": [{ "version": "FW1.8.18", "url": url }] }
    ] } })));
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let (status, body) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "trmnl" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    server.display_as(&key, &READINGS, "1.8.17", "og").await;
    let firmware = observer
        .wait(FIRMWARE, |firmware| {
            firmware["installed_version"] == "1.8.17"
        })
        .await;
    assert_eq!(firmware["latest_version"], "1.8.18");

    observer
        .publish("afterimage/41b41039a101/firmware/install", "install", false)
        .await;
    let firmware = observer
        .wait(FIRMWARE, |firmware| firmware["in_progress"] == true)
        .await;
    assert_eq!(firmware["installed_version"], "1.8.17");
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(body["device"]["firmware"]["update"]["state"], "requested");
    let answer = server.display_as(&key, &READINGS, "1.8.17", "og").await;
    assert_eq!(answer["update_firmware"], true);

    server.display_as(&key, &READINGS, "1.8.18", "og").await;
    let firmware: Value = observer
        .wait(FIRMWARE, |firmware| firmware["in_progress"] == false)
        .await;
    assert_eq!(firmware["installed_version"], "1.8.18");
}
