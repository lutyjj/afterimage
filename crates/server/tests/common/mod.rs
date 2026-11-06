//! The harness every integration test shares: the assembled server on a local
//! socket with its own data directory, a clock the test moves and scriptable
//! randomness, plus requests shaped as the firmware and the web UI send them.

#![allow(dead_code, unreachable_pub)]

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;

use clap::Parser;
use afterimage_server::app;
use afterimage_server::clock::Clock;
use afterimage_server::config::Config;
use afterimage_server::keys::{Entropy, OsEntropy};
use afterimage_server::secrets::Secrets;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;

pub struct ManualClock(AtomicI64);

impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl ManualClock {
    pub fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// Draws friendly IDs from a script, repeating the last one; keys stay random.
pub struct ScriptedIds(pub Mutex<VecDeque<[u8; 6]>>);

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

pub struct Server {
    pub url: String,
    pub clock: Arc<ManualClock>,
    pub data: TempDir,
    /// Keeps cookies, as a browser does.
    pub http: reqwest::Client,
    pub trmnl: Trmnl,
}

/// A stand-in for TRMNL's site: the firmware list, which lists no board until
/// a test sets one, and the flash images it names under `/flash/`.
#[derive(Clone)]
pub struct Trmnl {
    pub url: String,
    /// `None` answers 503.
    list: Arc<Mutex<Option<Value>>>,
    flash: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Requests for the list so far.
    pub list_requests: Arc<AtomicUsize>,
    /// Requests for flash images so far.
    pub flash_requests: Arc<AtomicUsize>,
    /// How long the list takes to answer.
    list_delay: Arc<Mutex<std::time::Duration>>,
    /// Flash images answer only while no test holds this.
    flash_gate: Arc<tokio::sync::RwLock<()>>,
}

impl Trmnl {
    async fn start() -> Self {
        async fn list(State(trmnl): State<Trmnl>) -> axum::response::Response {
            trmnl.list_requests.fetch_add(1, Ordering::SeqCst);
            let delay = *trmnl.list_delay.lock().unwrap();
            tokio::time::sleep(delay).await;
            match trmnl.list.lock().unwrap().clone() {
                Some(list) => axum::Json(list).into_response(),
                None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            }
        }
        async fn flash(
            State(trmnl): State<Trmnl>,
            Path(name): Path<String>,
        ) -> axum::response::Response {
            trmnl.flash_requests.fetch_add(1, Ordering::SeqCst);
            let _open = trmnl.flash_gate.read().await;
            match trmnl.flash.lock().unwrap().get(&name) {
                Some(bytes) => bytes.clone().into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let trmnl = Self {
            url: format!("http://{}", listener.local_addr().unwrap()),
            list: Arc::new(Mutex::new(Some(json!({ "data": { "models": [] } })))),
            flash: Arc::new(Mutex::new(HashMap::new())),
            list_requests: Arc::new(AtomicUsize::new(0)),
            flash_requests: Arc::new(AtomicUsize::new(0)),
            list_delay: Arc::new(Mutex::new(std::time::Duration::ZERO)),
            flash_gate: Arc::new(tokio::sync::RwLock::new(())),
        };
        let app = axum::Router::new()
            .route("/api/firmware/flash", get(list))
            .route("/flash/{name}", get(flash))
            .with_state(trmnl.clone());
        tokio::spawn(async move { axum::serve(listener, app).await });
        trmnl
    }

    pub fn set_list(&self, list: Option<Value>) {
        *self.list.lock().unwrap() = list;
    }

    pub fn delay_list(&self, delay: std::time::Duration) {
        *self.list_delay.lock().unwrap() = delay;
    }

    /// Holds every flash image's answer until the guard drops.
    pub async fn hold_flash(&self) -> tokio::sync::OwnedRwLockWriteGuard<()> {
        Arc::clone(&self.flash_gate).write_owned().await
    }

    /// Serves a flash image and returns its URL.
    pub fn publish(&self, name: &str, bytes: Vec<u8>) -> String {
        self.flash.lock().unwrap().insert(name.to_owned(), bytes);
        format!("{}/flash/{name}", self.url)
    }
}

pub const FIRMWARE: &str = "1.8.18";
pub const MODEL: &str = "og";
pub const UNKNOWN_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// What a device reports when nothing in particular is under test.
pub const READINGS: Readings = Readings {
    voltage: "4.12",
    percent: "69.4",
    rssi: "-61",
};

pub struct Readings {
    pub voltage: &'static str,
    pub percent: &'static str,
    pub rssi: &'static str,
}

pub async fn start() -> Server {
    start_with(Arc::new(OsEntropy), &[]).await
}

/// The server with this randomness and these extra command-line settings.
pub async fn start_with(entropy: Arc<dyn Entropy>, settings: &[&str]) -> Server {
    start_full(entropy, settings, Secrets::default()).await
}

/// The server with these secrets, in UTC.
pub async fn start_with_secrets(secrets: Secrets) -> Server {
    start_full(Arc::new(OsEntropy), &[], secrets).await
}

async fn start_full(entropy: Arc<dyn Entropy>, settings: &[&str], secrets: Secrets) -> Server {
    let data = TempDir::new().unwrap();
    let clock = Arc::new(ManualClock(AtomicI64::new(1_800_000_000)));
    let data_dir = data.path().to_str().unwrap().to_owned();
    let trmnl = Trmnl::start().await;
    let arguments = [
        "afterimage",
        "--data-dir",
        &data_dir,
        "--time-zone",
        "UTC",
        "--trmnl-url",
        &trmnl.url,
    ]
    .into_iter()
    .chain(settings.iter().copied());
    let config = Config::try_parse_from(arguments).unwrap();
    let app = app::build(&config, secrets, clock.clone(), entropy)
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        afterimage_device_api::serve(listener, tower::make::Shared::new(app)).await
    });
    Server {
        url,
        clock,
        data,
        http: reqwest::Client::builder()
            .tls_certs_only([])
            .cookie_store(true)
            .build()
            .unwrap(),
        trmnl,
    }
}

pub async fn json(response: reqwest::Response) -> Value {
    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap()
}

impl Server {
    /// `GET /api/setup` with `buildSetupHeaders`.
    pub async fn setup(&self, mac: &str) -> Value {
        self.setup_as(mac, MODEL).await
    }

    pub async fn setup_as(&self, mac: &str, model: &str) -> Value {
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
    pub async fn register(&self, mac: &str) -> (String, String) {
        let answer = self.setup(mac).await;
        assert_eq!(answer["status"], 200, "{answer}");
        (
            answer["api_key"].as_str().unwrap().to_owned(),
            answer["friendly_id"].as_str().unwrap().to_owned(),
        )
    }

    /// `GET /api/display` with `buildDisplayHeaders`, including the values whose
    /// format differs from TRMNL's published types.
    pub async fn display(&self, key: &str, readings: &Readings) -> Value {
        self.display_as(key, readings, FIRMWARE, MODEL).await
    }

    /// A check-in from this firmware release and model.
    pub async fn display_as(
        &self,
        key: &str,
        readings: &Readings,
        firmware: &str,
        model: &str,
    ) -> Value {
        let response = self
            .http
            .get(format!("{}/api/display", self.url))
            .header("ID", "41:B4:10:39:A1:01")
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
            .header("FW-Version", firmware)
            .header("Model", model)
            .header("Image-Cached", "false")
            .header("Wake-Time", "12")
            .header("RSSI", readings.rssi)
            .header("WiFi-Band", "2.4")
            .header("Temperature-Profile", "true")
            .header("Width", "800")
            .header("Height", "480")
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

    pub async fn current(&self, key: &str) -> Value {
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
    pub async fn log(&self, key: &str, body: String) -> StatusCode {
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

    pub async fn stored(&self, query: &'static str) -> Vec<sqlx::sqlite::SqliteRow> {
        let path = self.data.path().join("afterimage.db");
        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap();
        sqlx::query(query).fetch_all(&pool).await.unwrap()
    }

    /// Every file of the database, the write-ahead log included.
    pub fn database_files(&self) -> Vec<PathBuf> {
        std::fs::read_dir(self.data.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }
}

/// A log entry as `serialize_log` writes it.
pub fn log_entry(message: &str) -> Value {
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

pub fn mac(index: u8) -> String {
    format!("41:B4:10:39:A1:{index:02X}")
}

/// A management API call as the web UI makes it, with the session cookie this
/// server's client holds.
impl Server {
    pub async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let mut request = self.http.request(method, format!("{}{path}", self.url));
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let bytes = response.bytes().await.unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value)
    }

    /// Creates the first account and keeps its session.
    pub async fn sign_up(&self) -> Value {
        let (status, body) = self
            .call(
                "POST",
                "/manage/auth/first-account",
                Some(json!({ "name": "Ada", "username": "ada", "password": "correct horse battery" })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body["account"].clone()
    }

    /// Sets up a device, checks it in once (so the server knows its panel) and
    /// claims it; returns the device and its key.
    pub async fn claim(&self, mac: &str) -> (Value, String) {
        let (key, code) = self.register(mac).await;
        self.display(&key, &READINGS).await;
        let (status, body) = self
            .call("POST", "/manage/devices", Some(json!({ "code": code })))
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        (body["device"].clone(), key)
    }
}
