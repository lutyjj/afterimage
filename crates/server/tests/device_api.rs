//! The device API as TRMNL firmware sees it: requests carry the headers and bodies
//! firmware 1.8.18 builds (`lib/trmnl/src/api-client/request_headers.cpp`,
//! `serialize_log.cpp`), and answers are checked against what the firmware does
//! with each status value: on check-in, 202 shows the device's code and 500 wipes
//! its Wi-Fi settings (`display_session.cpp`, `bl.cpp`), so no answer here is 500.

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use afterimage_server::keys::token_hash;
use afterimage_server::registrations::{IDLE_SECONDS, UNCLAIMED_LIMIT, UNUSED_SECONDS};
use common::{FIRMWARE, MODEL};
use common::{
    READINGS, Readings, ScriptedIds, UNKNOWN_KEY, json, log_entry, mac, start, start_with,
};
use reqwest::StatusCode;
use serde_json::json;
use sqlx::Row;

#[tokio::test]
async fn setup_issues_a_key_and_a_claim_code() {
    let server = start().await;
    let answer = server.setup("41:b4:10:39:a1:24").await;
    assert_eq!(answer["status"], 200);
    assert_eq!(answer["api_key"].as_str().unwrap().len(), 43);
    let friendly_id = answer["friendly_id"].as_str().unwrap();
    assert_eq!(friendly_id.len(), 6);
    assert!(
        friendly_id
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
    );
    assert!(answer["message"].as_str().unwrap().contains("Claim"));
}

#[tokio::test]
async fn an_unclaimed_device_shows_its_code() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    for answer in [
        server.display(&key, &READINGS).await,
        server.current(&key).await,
    ] {
        assert_eq!(
            answer["status"], 202,
            "202 makes the firmware show its friendly ID"
        );
        assert!(
            answer["refresh_rate"].as_i64().unwrap() > 0,
            "the firmware reads a missing rate as 0"
        );
    }
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(
        answer["reset_firmware"], false,
        "true would wipe the device"
    );
    assert_eq!(answer["update_firmware"], false);
}

#[tokio::test]
async fn an_unknown_key_leaves_the_device_as_it_is() {
    let server = start().await;
    assert_eq!(server.display(UNKNOWN_KEY, &READINGS).await["status"], 202);
    assert_eq!(server.current(UNKNOWN_KEY).await["status"], 202);
    assert!(
        server
            .stored("SELECT id FROM registrations")
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_device_that_checks_in_keeps_its_key_for_good() {
    let server = start().await;
    let (key, friendly_id) = server.register(&mac(1)).await;
    for _ in 0..48 {
        server.clock.advance(60 * 60);
        assert_eq!(server.display(&key, &READINGS).await["status"], 202);
    }
    // Setup, which drops unused and idle registrations, leaves this one alone.
    server.register(&mac(2)).await;
    let rows = server.stored("SELECT friendly_id FROM registrations").await;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.get::<String, _>(0) == friendly_id)
    );
}

/// Firmware 1.8.18 answers 202 with its own fast-poll schedule (`fastPollSeconds`):
/// 50 polls at 5 s, 10 at 60 s, 10 at 900 s, then hourly. A setup with the same MAC
/// before every one of them takes nothing from the device.
#[tokio::test]
async fn setup_never_replaces_a_key() {
    let server = start().await;
    let (key, friendly_id) = server.register(&mac(1)).await;
    let schedule = [(50, 5), (10, 60), (10, 900), (6, 3600)];
    for (polls, seconds) in schedule {
        for _ in 0..polls {
            server.clock.advance(seconds - 1);
            let spoofed = server.setup(&mac(1)).await;
            assert_ne!(spoofed["friendly_id"], friendly_id.as_str());
            server.clock.advance(1);
            assert_eq!(server.display(&key, &READINGS).await["status"], 202);
        }
    }
    let rows = server
        .stored("SELECT friendly_id, seen_at, created_at FROM registrations")
        .await;
    let own = rows
        .iter()
        .find(|row| row.get::<String, _>(0) == friendly_id)
        .expect("the device's registration is still there");
    assert!(own.get::<Option<i64>, _>(1).unwrap() > own.get::<i64, _>(2));
}

#[tokio::test]
async fn setup_again_gives_the_device_a_new_code_at_once() {
    let server = start().await;
    let (old_key, old_id) = server.register(&mac(1)).await;
    server.display(&old_key, &READINGS).await;
    let (new_key, new_id) = server.register(&mac(1)).await;
    assert_ne!(new_key, old_key);
    assert_ne!(new_id, old_id);
    assert_eq!(server.display(&new_key, &READINGS).await["status"], 202);
    assert_eq!(server.display(&old_key, &READINGS).await["status"], 202);
    assert_eq!(server.stored("SELECT id FROM registrations").await.len(), 2);
}

#[tokio::test]
async fn concurrent_setups_each_get_their_own_key_and_code() {
    let server = Arc::new(start().await);
    let setups: Vec<_> = (0..8)
        .map(|_| {
            let server = Arc::clone(&server);
            tokio::spawn(async move { server.register(&mac(1)).await })
        })
        .collect();
    let mut answers = Vec::new();
    for setup in setups {
        answers.push(setup.await.unwrap());
    }
    let codes: std::collections::HashSet<_> = answers.iter().map(|(_, id)| id.clone()).collect();
    assert_eq!(codes.len(), 8);
    for (key, _) in &answers {
        assert_eq!(server.display(key, &READINGS).await["status"], 202);
    }
}

/// The unclaimed limit holds under concurrent setups: each setup counts and inserts
/// inside one write transaction.
#[tokio::test]
async fn concurrent_setups_at_the_limit_take_the_last_place_once() {
    let server = Arc::new(start().await);
    for index in 0..u8::try_from(UNCLAIMED_LIMIT - 1).unwrap() {
        server.register(&mac(index)).await;
    }
    let setups: Vec<_> = (0..8)
        .map(|index| {
            let server = Arc::clone(&server);
            tokio::spawn(async move { server.setup(&format!("02:00:00:00:01:{index:02X}")).await })
        })
        .collect();
    let mut registered = 0;
    for setup in setups {
        if setup.await.unwrap()["status"] == 200 {
            registered += 1;
        }
    }
    assert_eq!(registered, 1);
    assert_eq!(
        server.stored("SELECT id FROM registrations").await.len(),
        usize::try_from(UNCLAIMED_LIMIT).unwrap()
    );
}

/// A registration ends on time whether or not another setup runs: its key stops
/// working and a late check-in does not revive it.
#[tokio::test]
async fn an_ended_registration_does_not_come_back() {
    let server = start().await;
    let (unused, _) = server.register(&mac(1)).await;
    let (idle, _) = server.register(&mac(2)).await;
    server.display(&idle, &READINGS).await;
    server.clock.advance(UNUSED_SECONDS);
    server.display(&unused, &READINGS).await;
    let rows = server
        .stored("SELECT seen_at FROM registrations WHERE mac = '41:B4:10:39:A1:01'")
        .await;
    assert_eq!(rows[0].get::<Option<i64>, _>(0), None, "not revived");
    server.clock.advance(IDLE_SECONDS - UNUSED_SECONDS);
    assert_eq!(
        server.log(&idle, json!({ "logs": [] }).to_string()).await,
        StatusCode::UNAUTHORIZED,
        "an idle registration's key stops working"
    );
}

#[tokio::test]
async fn records_readings_and_keeps_the_last_known_value() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    server.display(&key, &READINGS).await;
    server.clock.advance(900);
    let garbled = Readings {
        voltage: "3000/3500",
        percent: "",
        rssi: "-58",
    };
    server.display(&key, &garbled).await;
    let row = &server
        .stored(
            "SELECT battery_voltage, battery_percent, signal_dbm, firmware_version, seen_at, created_at FROM registrations",
        )
        .await[0];
    assert_eq!(
        row.get::<Option<f64>, _>(0),
        Some(4.12),
        "an unreadable voltage keeps the last one"
    );
    assert_eq!(row.get::<Option<f64>, _>(1), Some(69.4));
    assert_eq!(row.get::<Option<i64>, _>(2), Some(-58));
    assert_eq!(row.get::<Option<String>, _>(3).as_deref(), Some(FIRMWARE));
    assert_eq!(
        row.get::<i64, _>(4) - row.get::<i64, _>(5),
        900,
        "seen at the last check-in"
    );
}

/// Firmware on every board but TRMNL X reports a voltage and no charge.
#[tokio::test]
async fn estimates_the_charge_from_the_voltage() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    let voltage_only = Readings {
        voltage: "3.5",
        percent: "",
        rssi: "-58",
    };
    server.display(&key, &voltage_only).await;
    let row = &server
        .stored("SELECT battery_percent FROM registrations")
        .await[0];
    // (3.5 V - 3.0 V) / 0.012 V per percent, TRMNL's formula.
    assert_eq!(row.get::<Option<f64>, _>(0), Some(42.0));
}

#[tokio::test]
async fn stores_only_a_hash_of_the_key() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    let files = server.database_files();
    assert!(
        files
            .iter()
            .any(|path| path.to_string_lossy().ends_with("-wal"))
    );
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(key.len())
                .any(|window| window == key.as_bytes()),
            "{} holds the key",
            path.display()
        );
    }
    let row = &server.stored("SELECT key_hash FROM registrations").await[0];
    assert_eq!(row.get::<Vec<u8>, _>(0), token_hash(&key));
}

#[tokio::test]
async fn refuses_setup_beyond_the_unclaimed_limit() {
    let server = start().await;
    let mut keys = Vec::new();
    for index in 0..u8::try_from(UNCLAIMED_LIMIT).unwrap() {
        keys.push(server.register(&mac(index)).await.0);
    }
    for mac in ["02:00:00:00:00:01".to_owned(), mac(0)] {
        let answer = server.setup(&mac).await;
        assert_eq!(
            answer["status"], 404,
            "404 makes the firmware show 'not registered'"
        );
        assert!(answer.get("api_key").is_none());
        assert!(
            answer["message"]
                .as_str()
                .unwrap()
                .contains("waiting to be claimed")
        );
    }
    assert_eq!(server.display(&keys[0], &READINGS).await["status"], 202);
}

#[tokio::test]
async fn an_unused_registration_frees_its_place_after_a_day() {
    let server = start().await;
    let mut keys = Vec::new();
    for index in 0..u8::try_from(UNCLAIMED_LIMIT).unwrap() {
        keys.push(server.register(&mac(index)).await.0);
    }
    server.display(&keys[1], &READINGS).await;
    server.clock.advance(UNUSED_SECONDS - 1);
    assert_eq!(
        server.setup("02:00:00:00:00:01").await["status"],
        404,
        "unused registrations hold their places for a day"
    );
    server.clock.advance(1);
    assert_eq!(server.setup("02:00:00:00:00:01").await["status"], 200);
    assert_eq!(
        server.stored("SELECT id FROM registrations").await.len(),
        2,
        "the device that checked in stays"
    );
}

#[tokio::test]
async fn an_idle_registration_frees_its_place_after_a_week() {
    let server = start().await;
    let mut keys = Vec::new();
    for index in 0..u8::try_from(UNCLAIMED_LIMIT).unwrap() {
        let (key, _) = server.register(&mac(index)).await;
        server.display(&key, &READINGS).await;
        keys.push(key);
    }
    server.clock.advance(IDLE_SECONDS - 1);
    server.display(&keys[0], &READINGS).await;
    assert_eq!(server.setup("02:00:00:00:00:01").await["status"], 404);
    server.clock.advance(1);
    assert_eq!(server.setup("02:00:00:00:00:01").await["status"], 200);
    assert_eq!(
        server.stored("SELECT id FROM registrations").await.len(),
        2,
        "the registration that kept checking in stays"
    );
}

#[tokio::test]
async fn a_setup_without_a_model_registers_and_its_check_in_names_it() {
    // Firmware before 1.8 sends `Model` empty at setup, and names it at check-in.
    let server = start().await;
    server.sign_up().await;
    let answer = server.setup_as(&mac(1), "").await;
    assert_eq!(answer["status"], 200, "{answer}");
    let key = answer["api_key"].as_str().unwrap().to_owned();
    let code = answer["friendly_id"].as_str().unwrap().to_owned();
    server.display(&key, &READINGS).await;
    let (status, claimed) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{claimed}");
    assert_eq!(claimed["device"]["firmwareModel"], MODEL);
}

#[tokio::test]
async fn refuses_setup_it_cannot_read() {
    let server = start().await;
    let answer = server.setup("not a mac").await;
    assert_eq!(answer["status"], 404);
    assert!(answer["message"].as_str().unwrap().contains("MAC address"));
    let answer = server.setup_as(&mac(1), &"x".repeat(65)).await;
    assert_eq!(answer["status"], 404);
    assert!(answer["message"].as_str().unwrap().contains("model"));
    assert!(
        server
            .stored("SELECT id FROM registrations")
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn draws_another_code_when_one_is_taken() {
    let taken = *b"AAAAAA";
    let server = start_with(
        Arc::new(ScriptedIds(Mutex::new(VecDeque::from([
            taken, taken, *b"BBBBBB",
        ])))),
        &[],
    )
    .await;
    let (_, first) = server.register(&mac(1)).await;
    let (_, second) = server.register(&mac(2)).await;
    assert_ne!(first, second);
}

/// A server failure is HTTP 500 with no body `status`: the firmware retries an
/// HTTP error, while a body `status` of 500 would wipe the device.
#[tokio::test]
async fn a_server_failure_is_an_http_error_the_firmware_retries() {
    let server = start_with(
        Arc::new(ScriptedIds(Mutex::new(VecDeque::from([*b"AAAAAA"])))),
        &[],
    )
    .await;
    server.register(&mac(1)).await;
    let response = server
        .http
        .get(format!("{}/api/setup", server.url))
        .header("ID", mac(2))
        .header("Model", MODEL)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json(response).await;
    assert!(body.get("status").is_none(), "{body}");
}

#[tokio::test]
async fn takes_firmware_log_batches_from_known_devices() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    let batch = json!({ "logs": [log_entry("Wifi connected"), log_entry("Display refreshed")] });
    assert_eq!(
        server.log(&key, batch.to_string()).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        server.log(UNKNOWN_KEY, batch.to_string()).await,
        StatusCode::UNAUTHORIZED
    );
}

/// The generated deserializer stops reading at the codegen limit and answers 400
/// (`BufferHttpBodyBytes`); a body of the same entries just under it is taken.
#[tokio::test]
async fn refuses_log_bodies_over_64_kib() {
    let server = start().await;
    let (key, _) = server.register(&mac(1)).await;
    let batch = |count: usize| {
        let entries: Vec<_> = (0..count).map(|_| log_entry(&"x".repeat(512))).collect();
        json!({ "logs": entries }).to_string()
    };
    let (under, over) = (batch(60), batch(80));
    assert!(
        under.len() < 64 * 1024 && over.len() > 64 * 1024,
        "{} {}",
        under.len(),
        over.len()
    );
    assert_eq!(server.log(&key, under).await, StatusCode::NO_CONTENT);
    assert_eq!(server.log(&key, over).await, StatusCode::BAD_REQUEST);
}
