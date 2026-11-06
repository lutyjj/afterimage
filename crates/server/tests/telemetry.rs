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
    let observer = Observer::start(&broker);
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
    // TRMNL names no board og, so there is no newest release, and Home
    // Assistant's update entity refuses a null version.
    let firmware = observer.wait(FIRMWARE, |_| true).await;
    assert_eq!(
        firmware,
        json!({ "installed_version": "1.8.18", "in_progress": false, "title": "TRMNL firmware" })
    );

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
    let observer = Observer::start(&broker);
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
    let observer = Observer::start(&broker);
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
    let observer = Observer::start(&broker);
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

/// Home Assistant drops a discovery payload with a null where it reads a string,
/// and an update state with a null version.
fn assert_no_nulls(value: &Value, at: &str) {
    match value {
        Value::Null => panic!("null at {at}"),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                assert_no_nulls(item, &format!("{at}/{index}"));
            }
        }
        Value::Object(fields) => {
            for (name, field) in fields {
                assert_no_nulls(field, &format!("{at}/{name}"));
            }
        }
        _ => {}
    }
}

/// A device whose firmware has not reported its version or model yet.
#[tokio::test]
async fn unknown_facts_are_left_out_rather_than_null() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let server = start(&broker).await;
    server.sign_up().await;
    let setup = server.setup_as(&mac(1), "").await;
    let key = setup["api_key"].as_str().unwrap();
    let code = setup["friendly_id"].as_str().unwrap();
    server
        .http
        .get(format!("{}/api/display", server.url))
        .header("Access-Token", key)
        .send()
        .await
        .unwrap();
    let (status, _) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let config = observer.wait(CONFIG, |_| true).await;
    assert_no_nulls(&config, "config");
    assert!(config["device"].get("sw_version").is_none());
    assert!(config["device"].get("model").is_none());
    let firmware = observer.wait(FIRMWARE, |_| true).await;
    assert_no_nulls(&firmware, "firmware");
}

/// What a device removed while the server was away left on the broker goes once
/// the server connects.
#[tokio::test]
async fn retained_topics_of_a_gone_device_are_cleared_on_connecting() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let ghost_config = "homeassistant/device/afterimage_aabbccddeeff/config";
    let ghost_state = "afterimage/aabbccddeeff/state";
    observer
        .publish(ghost_config, r#"{"device":{}}"#, true)
        .await;
    observer
        .publish(ghost_state, r#"{"battery":50}"#, true)
        .await;
    observer.wait(ghost_state, |_| true).await;
    let server = start(&broker).await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    observer.gone(ghost_config).await;
    observer.gone(ghost_state).await;
    observer.wait(CONFIG, |_| true).await;
}

/// A retained install would replay at every connection, so it is ignored.
#[tokio::test]
async fn a_retained_install_command_is_ignored() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    observer
        .publish("afterimage/41b41039a101/firmware/install", "install", true)
        .await;
    let server = start(&broker).await;
    let url = server
        .trmnl
        .publish("og.bin", flash_image(&app_image(b"firmware")));
    server.trmnl.set_list(Some(json!({ "data": { "models": [
        { "keyname": "og", "chipFamily": "ESP32-C3", "label": "OG",
          "versions": [{ "version": "FW1.8.18", "url": url }] }
    ] } })));
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    // Home Assistant's announcement goes through the same queue after the
    // retained command, so once its republish shows, the command was handled.
    observer.wait(CONFIG, |_| true).await;
    observer.publish(CONFIG, "", true).await;
    observer.gone(CONFIG).await;
    observer
        .publish("homeassistant/status", "online", false)
        .await;
    observer.wait(CONFIG, |_| true).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{}", device["id"]), None)
        .await;
    assert!(body["device"]["firmware"]["update"].is_null(), "{body}");
}

#[tokio::test]
async fn a_restarted_broker_gets_every_device_again() {
    let broker = Broker::start().await;
    let port = broker.port;
    let observer = Observer::start(&broker);
    let server = start(&broker).await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    observer.wait(CONFIG, |_| true).await;
    drop(broker);
    let _restarted = Broker::start_on(port).await;
    // The new broker keeps nothing, so a discovery payload the observer reads
    // after it reconnected is one the server published again.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while observer
        .connections
        .load(std::sync::atomic::Ordering::SeqCst)
        < 2
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the observer never reconnected"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    observer.wait(CONFIG, |_| true).await;
    assert_eq!(observer.wait("afterimage/status", |_| true).await, "online");
}

/// A device set up again and claimed by another account leaves a device of the
/// first account on its old registration. The topics follow the device that
/// checks in, and removing the stale one leaves them published.
#[tokio::test]
async fn one_mac_in_two_accounts_publishes_the_device_that_checks_in() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let server = start(&broker).await;
    server.sign_up().await;
    let (stale, _) = server.claim(&mac(1)).await;

    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    let hash = password_auth::generate_hash("second password here");
    sqlx::query("INSERT INTO accounts (name, username, password_hash, created_at) VALUES ('Eve', 'eve', ?, 0)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();
    let eve = reqwest::Client::builder()
        .tls_certs_only([])
        .cookie_store(true)
        .build()
        .unwrap();
    let signed_in = eve
        .post(format!("{}/manage/auth/password", server.url))
        .header("Content-Type", "application/json")
        .body(json!({ "username": "eve", "password": "second password here" }).to_string())
        .send()
        .await
        .unwrap();
    assert!(signed_in.status().is_success());

    server.clock.advance(60);
    let (key, code) = server.register(&mac(1)).await;
    server.display(&key, &READINGS).await;
    let claimed: Value = eve
        .post(format!("{}/manage/devices", server.url))
        .header("Content-Type", "application/json")
        .body(json!({ "code": code, "name": "Eve's display" }).to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(claimed["device"]["name"], "Eve's display");
    observer
        .wait(CONFIG, |config| config["device"]["name"] == "Eve's display")
        .await;

    let (status, _) = server
        .call("DELETE", &format!("/manage/devices/{}", stale["id"]), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    server.clock.advance(60);
    server.display(&key, &READINGS).await;
    let config = observer.wait(CONFIG, |_| true).await;
    assert_eq!(config["device"]["name"], "Eve's display", "still published");
}

/// rumqttc's own TLS setup panics without a process-wide crypto provider; the
/// server sets up TLS itself and serves while the broker is out of reach.
#[tokio::test]
async fn a_tls_broker_url_starts_the_server() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("mqtts://127.0.0.1:{closed}");
    let server = start_with(Arc::new(OsEntropy), &["--mqtt-url", &url]).await;
    let response = server
        .http
        .get(format!("{}/manage/auth", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
