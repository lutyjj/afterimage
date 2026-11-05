//! The device API as TRMNL firmware sees it: requests carry the headers and bodies
//! firmware 1.8.18 builds (`lib/trmnl/src/api-client/request_headers.cpp`,
//! `serialize_log.cpp`), and answers are checked against what the firmware does
//! with each status value: on check-in, 202 shows the device's code and 500 wipes
//! its Wi-Fi settings (`display_session.cpp`, `bl.cpp`), so no answer here is 500.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use afterimage_server::clock::Clock;
use afterimage_server::device_api::serve;
use afterimage_server::devices::{QUIET_SECONDS, Registry, UNCLAIMED_LIMIT, UNUSED_KEY_SECONDS};
use afterimage_server::secrets::{Entropy, OsEntropy, key_hash};
use reqwest::StatusCode;
use serde_json::{Value, json};
use sqlx::Row;
use tempfile::TempDir;
use tokio::net::TcpListener;

struct ManualClock(AtomicI64);

impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl ManualClock {
    fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// Draws friendly IDs from a script, repeating the last one; keys stay random.
struct ScriptedIds(Mutex<VecDeque<[u8; 6]>>);

impl Entropy for ScriptedIds {
    fn fill(&self, buffer: &mut [u8]) {
        if buffer.len() != 6 {
            return OsEntropy.fill(buffer);
        }
        let mut script = self.0.lock().unwrap();
        let next = if script.len() > 1 {
            script.pop_front().unwrap()
        } else {
            script[0]
        };
        buffer.copy_from_slice(&next);
    }
}

struct Server {
    url: String,
    clock: Arc<ManualClock>,
    data: TempDir,
    http: reqwest::Client,
}

const FIRMWARE: &str = "1.8.18";
const MODEL: &str = "og";
const UNKNOWN_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// What a device reports when nothing in particular is under test.
const READINGS: Readings = Readings {
    voltage: "4.12",
    percent: "69.4",
    rssi: "-61",
};

struct Readings {
    voltage: &'static str,
    percent: &'static str,
    rssi: &'static str,
}

async fn start() -> Server {
    start_with(Arc::new(OsEntropy)).await
}

async fn start_with(entropy: Arc<dyn Entropy>) -> Server {
    let data = TempDir::new().unwrap();
    let clock = Arc::new(ManualClock(AtomicI64::new(1_800_000_000)));
    let registry = Registry::open(&data.path().join("afterimage.db"), clock.clone(), entropy)
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, Arc::new(registry), std::future::pending()));
    Server {
        url,
        clock,
        data,
        http: reqwest::Client::new(),
    }
}

async fn json(response: reqwest::Response) -> Value {
    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap()
}

impl Server {
    /// `GET /api/setup` with `buildSetupHeaders`.
    async fn setup(&self, mac: &str) -> Value {
        self.setup_as(mac, MODEL).await
    }

    async fn setup_as(&self, mac: &str, model: &str) -> Value {
        let response = self
            .http
            .get(format!("{}/api/setup", self.url))
            .header("ID", mac)
            .header("Content-Type", "application/json")
            .header("FW-Version", FIRMWARE)
            .header("Model", model)
            .header("Width", "800")
            .header("Height", "480")
            .header("RSSI", "-61")
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "setup is HTTP 200 whatever the outcome"
        );
        json(response).await
    }

    /// Sets up a new device and returns its key and friendly ID.
    async fn register(&self, mac: &str) -> (String, String) {
        let answer = self.setup(mac).await;
        assert_eq!(answer["status"], 200, "{answer}");
        (
            answer["api_key"].as_str().unwrap().to_owned(),
            answer["friendly_id"].as_str().unwrap().to_owned(),
        )
    }

    /// `GET /api/display` with `buildDisplayHeaders`, including the values whose
    /// format differs from TRMNL's published types.
    async fn display(&self, key: &str, readings: &Readings) -> Value {
        let response = self
            .http
            .get(format!("{}/api/display", self.url))
            .header("ID", "41:B4:10:39:A1:24")
            .header("Content-Type", "application/json")
            .header("Update-Source", "timer")
            .header("Access-Token", key)
            .header("Refresh-Rate", "900")
            .header("Battery-Voltage", readings.voltage)
            .header("Battery-Charging", "0")
            .header("USB-Connected", "false")
            .header("Battery-Count", "1")
            .header("Percent-Charged", readings.percent)
            .header("Battery-Health", "-1")
            .header("Battery-Current", "-1")
            .header("Battery-Temp", "-1")
            .header("Battery-Capacity", "3000/3500")
            .header("Gauge-SOC", "-1")
            .header("Gauge-Health", "-1")
            .header("Gauge-Capacity", "-1/-1")
            .header("FW-Version", FIRMWARE)
            .header("Model", MODEL)
            .header("Image-Cached", "false")
            .header("Wake-Time", "12")
            .header("Reset-Reason", "deepsleep")
            .header("RSSI", readings.rssi)
            .header("WiFi-Band", "2.4")
            .header("Temperature-Profile", "true")
            .header("Width", "800")
            .header("Height", "480")
            .header("Log-Level", "info")
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "a check-in is HTTP 200 whatever the outcome"
        );
        json(response).await
    }

    async fn current(&self, key: &str) -> Value {
        let response = self
            .http
            .get(format!("{}/api/display/current", self.url))
            .header("ID", "41:B4:10:39:A1:24")
            .header("Access-Token", key)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        json(response).await
    }

    /// `POST /api/log` with `buildLogHeaders` and the given body.
    async fn log(&self, key: &str, body: String) -> StatusCode {
        self.http
            .post(format!("{}/api/log", self.url))
            .header("ID", "41:B4:10:39:A1:24")
            .header("Accept", "application/json, */*")
            .header("Access-Token", key)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap()
            .status()
    }

    async fn stored(&self, query: &'static str) -> Vec<sqlx::sqlite::SqliteRow> {
        let path = self.data.path().join("afterimage.db");
        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap();
        sqlx::query(query).fetch_all(&pool).await.unwrap()
    }

    /// Every file of the database, the write-ahead log included.
    fn database_files(&self) -> Vec<PathBuf> {
        std::fs::read_dir(self.data.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }
}

/// A log entry as `serialize_log` writes it.
fn log_entry(message: &str) -> Value {
    json!({
        "created_at": 1_762_370_000,
        "id": 7,
        "message": message,
        "source_line": 88,
        "source_path": "src/bl.cpp",
        "wifi_signal": -61,
        "wifi_status": "connected",
        "refresh_rate": 900,
        "sleep_duration": 899,
        "firmware_version": FIRMWARE,
        "special_function": "none",
        "battery_voltage": 4.12,
        "wake_reason": "timer",
        "free_heap_size": 120_000,
        "max_alloc_size": 64_000,
        "level": "info"
    })
}

fn mac(index: u8) -> String {
    format!("41:B4:10:39:A1:{index:02X}")
}

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
    assert!(server.stored("SELECT id FROM devices").await.is_empty());
}

#[tokio::test]
async fn a_device_that_checks_in_keeps_its_key_for_good() {
    let server = start().await;
    let (key, friendly_id) = server.register(&mac(1)).await;
    for _ in 0..48 {
        server.clock.advance(60 * 60);
        assert_eq!(server.display(&key, &READINGS).await["status"], 202);
    }
    // Setup, which drops unused registrations, leaves this one alone.
    server.register(&mac(2)).await;
    let rows = server.stored("SELECT friendly_id FROM devices").await;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.get::<String, _>(0) == friendly_id)
    );
}

#[tokio::test]
async fn setup_again_waits_until_the_key_is_quiet() {
    let server = start().await;
    let (old_key, old_id) = server.register(&mac(1)).await;
    server.display(&old_key, &READINGS).await;

    server.clock.advance(QUIET_SECONDS - 1);
    let answer = server.setup(&mac(1)).await;
    assert_eq!(
        answer["status"], 404,
        "a spoofed MAC cannot take a running device's key"
    );
    assert!(answer.get("api_key").is_none());
    assert!(answer["message"].as_str().unwrap().contains("30 minutes"));
    assert_eq!(server.display(&old_key, &READINGS).await["status"], 202);

    server.clock.advance(QUIET_SECONDS);
    let (new_key, new_id) = server.register(&mac(1)).await;
    assert_ne!(new_key, old_key);
    assert_ne!(new_id, old_id, "a new key comes with a new code");

    server.display(&old_key, &READINGS).await;
    let row = &server.stored("SELECT seen_at FROM devices").await[0];
    assert_eq!(
        row.get::<Option<i64>, _>(0),
        None,
        "the old key no longer reaches the device"
    );
    server.display(&new_key, &READINGS).await;
    let row = &server.stored("SELECT seen_at FROM devices").await[0];
    assert!(row.get::<Option<i64>, _>(0).is_some());
}

#[tokio::test]
async fn concurrent_setups_of_one_device_register_it_once() {
    let server = Arc::new(start().await);
    let setups: Vec<_> = (0..8)
        .map(|_| {
            let server = Arc::clone(&server);
            tokio::spawn(async move { server.setup(&mac(1)).await })
        })
        .collect();
    let mut keys = Vec::new();
    for setup in setups {
        let answer = setup.await.unwrap();
        match answer["status"].as_i64() {
            Some(200) => keys.push(answer["api_key"].as_str().unwrap().to_owned()),
            Some(404) => {}
            other => panic!("unexpected setup status {other:?}"),
        }
    }
    assert_eq!(
        keys.len(),
        1,
        "one setup registers it; the rest find its key in use"
    );
    assert_eq!(server.stored("SELECT id FROM devices").await.len(), 1);
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
            "SELECT battery_voltage, battery_percent, signal_dbm, firmware_version, seen_at, created_at FROM devices",
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
    let row = &server.stored("SELECT key_hash FROM devices").await[0];
    assert_eq!(row.get::<Vec<u8>, _>(0), key_hash(&key));
}

#[tokio::test]
async fn refuses_new_devices_beyond_the_unclaimed_limit() {
    let server = start().await;
    for index in 0..u8::try_from(UNCLAIMED_LIMIT).unwrap() {
        server.register(&mac(index)).await;
    }
    let answer = server.setup("02:00:00:00:00:01").await;
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
    server.clock.advance(QUIET_SECONDS);
    let (key, _) = server.register(&mac(0)).await;
    assert_eq!(
        server.display(&key, &READINGS).await["status"],
        202,
        "a known device can still set up again"
    );
}

#[tokio::test]
async fn an_unused_registration_frees_its_place_after_a_day() {
    let server = start().await;
    let mut keys = Vec::new();
    for index in 0..u8::try_from(UNCLAIMED_LIMIT).unwrap() {
        keys.push(server.register(&mac(index)).await.0);
    }
    server.display(&keys[1], &READINGS).await;
    server.clock.advance(UNUSED_KEY_SECONDS - 1);
    assert_eq!(
        server.setup("02:00:00:00:00:01").await["status"],
        404,
        "unused registrations hold their places for a day"
    );
    server.clock.advance(1);
    assert_eq!(server.setup("02:00:00:00:00:01").await["status"], 200);
    assert_eq!(
        server.stored("SELECT id FROM devices").await.len(),
        2,
        "the device that checked in stays"
    );
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
    assert!(server.stored("SELECT id FROM devices").await.is_empty());
}

#[tokio::test]
async fn draws_another_code_when_one_is_taken() {
    let taken = *b"AAAAAA";
    let server = start_with(Arc::new(ScriptedIds(Mutex::new(VecDeque::from([
        taken, taken, *b"BBBBBB",
    ])))))
    .await;
    let (_, first) = server.register(&mac(1)).await;
    let (_, second) = server.register(&mac(2)).await;
    assert_ne!(first, second);
}

/// A server failure is HTTP 500 with no body `status`: the firmware retries an
/// HTTP error, while a body `status` of 500 would wipe the device.
#[tokio::test]
async fn a_server_failure_is_an_http_error_the_firmware_retries() {
    let server = start_with(Arc::new(ScriptedIds(Mutex::new(VecDeque::from([
        *b"AAAAAA",
    ])))))
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
