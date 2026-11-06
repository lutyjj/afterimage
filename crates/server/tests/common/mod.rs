//! The harness every integration test shares: the assembled server on a local
//! socket with its own data directory, a clock the test moves and scriptable
//! randomness, plus requests shaped as the firmware and the web UI send them.

#![allow(dead_code, unreachable_pub)]

pub mod oidc;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;

use clap::Parser;
use esp_idf_part::{AppType, DataType, Flags, Partition, PartitionTable, SubType, Type};
use afterimage_server::app;
use afterimage_server::clock::Clock;
use afterimage_server::config::Config;
use afterimage_server::keys::{Entropy, OsEntropy};
use afterimage_server::secrets::Secrets;
use reqwest::StatusCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
    /// Shared by another server on the same data, as after a restart.
    pub data: Arc<TempDir>,
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

/// The server signing in through a stand-in OIDC provider at the server's time,
/// with these extra settings.
pub async fn start_with_provider(settings: &[&str]) -> (Server, oidc::Provider) {
    let clock = new_clock();
    let provider = oidc::Provider::start(clock.clone()).await;
    let provider_settings = provider.settings();
    let settings: Vec<&str> = provider_settings
        .iter()
        .map(String::as_str)
        .chain(settings.iter().copied())
        .collect();
    let server = start_at(clock, Arc::new(OsEntropy), &settings, Secrets::default()).await;
    (server, provider)
}

fn new_clock() -> Arc<ManualClock> {
    Arc::new(ManualClock(AtomicI64::new(1_800_000_000)))
}

async fn start_full(entropy: Arc<dyn Entropy>, settings: &[&str], secrets: Secrets) -> Server {
    start_at(new_clock(), entropy, settings, secrets).await
}

/// The server on a data directory the test prepared, such as a database
/// migrated to an earlier schema.
pub async fn start_on_data(data: TempDir) -> Server {
    start_on(
        Arc::new(data),
        new_clock(),
        Arc::new(OsEntropy),
        &[],
        Secrets::default(),
    )
    .await
}

impl Server {
    /// Another handle on this server, with this one's cookies.
    pub fn clone_handle(&self) -> Server {
        Server {
            url: self.url.clone(),
            clock: Arc::clone(&self.clock),
            data: Arc::clone(&self.data),
            http: self.http.clone(),
            trmnl: self.trmnl.clone(),
        }
    }

    /// Another server on this one's data and clock, as the server is after a
    /// restart, with these settings.
    pub async fn again(&self, settings: &[&str]) -> Server {
        start_on(
            Arc::clone(&self.data),
            Arc::clone(&self.clock),
            Arc::new(OsEntropy),
            settings,
            Secrets::default(),
        )
        .await
    }
}

async fn start_at(
    clock: Arc<ManualClock>,
    entropy: Arc<dyn Entropy>,
    settings: &[&str],
    secrets: Secrets,
) -> Server {
    start_on(
        Arc::new(TempDir::new().unwrap()),
        clock,
        entropy,
        settings,
        secrets,
    )
    .await
}

async fn start_on(
    data: Arc<TempDir>,
    clock: Arc<ManualClock>,
    entropy: Arc<dyn Entropy>,
    settings: &[&str],
    secrets: Secrets,
) -> Server {
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

/// An app image of one segment holding `payload`, in ESP-IDF's format: the
/// header with `hash_appended` set, the segment, the checksum on the last byte
/// of its 16-byte block, and the SHA-256 of everything before.
pub fn app_image(payload: &[u8]) -> Vec<u8> {
    let mut image = vec![0; 24];
    image[0] = 0xE9;
    image[1] = 1;
    image[23] = 1;
    image.extend_from_slice(&0x3c00_0020_u32.to_le_bytes());
    image.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    image.extend_from_slice(payload);
    let checksum = payload.iter().fold(0xEF, |checksum, byte| checksum ^ byte);
    image.resize((image.len() + 1).next_multiple_of(16) - 1, 0);
    image.push(checksum);
    let digest = Sha256::digest(&image);
    image.extend_from_slice(&digest);
    image
}

/// A flash image with `app` in `ota_0` at 0x10000.
pub fn flash_image(app: &[u8]) -> Vec<u8> {
    let partition = |name: &str, ty, subtype, offset, size| {
        Partition::new(name, ty, subtype, offset, size, Flags::empty())
    };
    let table = PartitionTable::new(vec![
        partition(
            "nvs",
            Type::Data,
            SubType::Data(DataType::Nvs),
            0x9000,
            0x5000,
        ),
        partition(
            "otadata",
            Type::Data,
            SubType::Data(DataType::Ota),
            0xe000,
            0x2000,
        ),
        partition(
            "app0",
            Type::App,
            SubType::App(AppType::Ota_0),
            0x10000,
            0x10000,
        ),
        partition(
            "app1",
            Type::App,
            SubType::App(AppType::Ota_1),
            0x20000,
            0x10000,
        ),
    ]);
    let mut flash = vec![0xFF; 0x8000];
    flash.extend_from_slice(&table.to_bin().unwrap());
    flash.resize(0x10000, 0xFF);
    flash.extend_from_slice(app);
    flash
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

/// A Mosquitto broker of the test's own on a free local port, stopped when the
/// test drops it. `mosquitto` comes from the development shell, as in the Nix
/// build's checks.
pub struct Broker {
    pub port: u16,
    child: std::process::Child,
    _directory: TempDir,
}

/// Who a broker lets in.
enum Access<'a> {
    Anonymous,
    /// Nobody: no anonymous client and no user.
    Nobody,
    /// These users, by name and password, and nobody else.
    Users(&'a [(&'a str, &'a str)]),
}

impl Broker {
    pub async fn start() -> Self {
        Self::start_free(&Access::Anonymous).await
    }

    /// A broker on `port`, as after a restart; it keeps no retained messages.
    pub async fn start_on(port: u16) -> Self {
        Self::try_start(port, &Access::Anonymous)
            .await
            .unwrap_or_else(|| panic!("mosquitto could not listen on {port}"))
    }

    /// A broker that refuses every client.
    pub async fn start_refusing() -> Self {
        Self::start_free(&Access::Nobody).await
    }

    /// A broker that lets in only these users.
    pub async fn start_with_users(users: &[(&str, &str)]) -> Self {
        Self::start_free(&Access::Users(users)).await
    }

    /// A broker on a free port: one that another test's broker took between
    /// finding it and binding it is given up for another.
    async fn start_free(access: &Access<'_>) -> Self {
        loop {
            let port = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            if let Some(broker) = Self::try_start(port, access).await {
                return broker;
            }
        }
    }

    /// The broker on `port`, or `None` when its mosquitto could not keep the
    /// port, so that whatever answers there is not this test's.
    async fn try_start(port: u16, access: &Access<'_>) -> Option<Self> {
        let directory = TempDir::new().unwrap();
        let config = directory.path().join("mosquitto.conf");
        let mut text = format!("listener {port} 127.0.0.1\npersistence false\n");
        match access {
            Access::Anonymous => text.push_str("allow_anonymous true\n"),
            Access::Nobody => text.push_str("allow_anonymous false\n"),
            Access::Users(users) => {
                let passwords = directory.path().join("passwords");
                for (at, (user, password)) in users.iter().enumerate() {
                    let mut command = std::process::Command::new("mosquitto_passwd");
                    command.arg("-b");
                    if at == 0 {
                        command.arg("-c");
                    }
                    let status = command
                        .arg(&passwords)
                        .arg(user)
                        .arg(password)
                        .status()
                        .expect("mosquitto_passwd is on PATH, as in `nix develop`");
                    assert!(status.success());
                }
                text.push_str("allow_anonymous false\npassword_file ");
                text.push_str(&passwords.display().to_string());
                text.push('\n');
            }
        }
        std::fs::write(&config, text).unwrap();
        let mut child = std::process::Command::new("mosquitto")
            .arg("-c")
            .arg(&config)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("mosquitto is on PATH, as in `nix develop`");
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err()
        {
            if child.try_wait().unwrap().is_some() {
                return None;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "mosquitto never listened"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        // A mosquitto that lost the port to another exits at once.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if child.try_wait().unwrap().is_some() {
            return None;
        }
        Some(Self {
            port,
            child,
            _directory: directory,
        })
    }

    pub fn url(&self) -> String {
        format!("mqtt://127.0.0.1:{}", self.port)
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A topic and the payload it carried.
type Message = (String, Vec<u8>);

/// A client that subscribes to every topic and keeps the last message of each,
/// as Home Assistant reads retained topics; an empty retained message removes a
/// topic, as it does there.
pub struct Observer {
    pub client: rumqttc::AsyncClient,
    messages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Every message in the order it arrived, an emptied topic included.
    history: Arc<Mutex<Vec<Message>>>,
    /// Connections to the broker so far.
    pub connections: Arc<AtomicUsize>,
}

impl Observer {
    pub fn start(broker: &Broker) -> Self {
        Self::start_as(broker, None)
    }

    /// An observer that signs in to the broker with this user and password.
    pub fn start_as(broker: &Broker, credentials: Option<(&str, &str)>) -> Self {
        let mut options = rumqttc::MqttOptions::new("observer", "127.0.0.1", broker.port);
        options.set_max_packet_size(1024 * 1024, 1024 * 1024);
        if let Some((user, password)) = credentials {
            options.set_credentials(user, password);
        }
        let (client, mut eventloop) = rumqttc::AsyncClient::new(options, 64);
        let messages = Arc::new(Mutex::new(HashMap::new()));
        let kept = Arc::clone(&messages);
        let history = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&history);
        let connections = Arc::new(AtomicUsize::new(0));
        let connected = Arc::clone(&connections);
        let subscriber = client.clone();
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    // Every connection starts afresh and subscribes again, as Home
                    // Assistant reads a restarted broker anew.
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                        kept.lock().unwrap().clear();
                        connected.fetch_add(1, Ordering::SeqCst);
                        let _ = subscriber.try_subscribe("#", rumqttc::QoS::AtLeastOnce);
                    }
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(message))) => {
                        recorded
                            .lock()
                            .unwrap()
                            .push((message.topic.clone(), message.payload.to_vec()));
                        let mut kept = kept.lock().unwrap();
                        if message.payload.is_empty() {
                            kept.remove(&message.topic);
                        } else {
                            kept.insert(message.topic, message.payload.to_vec());
                        }
                    }
                    Ok(_) => {}
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
                }
            }
        });
        Self {
            client,
            messages,
            history,
            connections,
        }
    }

    /// The last message on `topic` once `accept` takes it, as JSON when it is.
    pub async fn wait(&self, topic: &str, accept: impl Fn(&Value) -> bool) -> Value {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let message = self.messages.lock().unwrap().get(topic).cloned();
            if let Some(bytes) = message {
                let value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                    Value::String(String::from_utf8_lossy(&bytes).into_owned())
                });
                if accept(&value) {
                    return value;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "no accepted message on {topic}: {:?}",
                self.messages
                    .lock()
                    .unwrap()
                    .get(topic)
                    .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// Every payload `topic` carried, in order, as text; an emptied topic reads
    /// as an empty string.
    pub fn payloads(&self, topic: &str) -> Vec<String> {
        self.history
            .lock()
            .unwrap()
            .iter()
            .filter(|(seen, _)| seen == topic)
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .collect()
    }

    /// Waits until `topic` holds nothing, as after the server cleared it.
    pub async fn wait_gone(&self, topic: &str) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.messages.lock().unwrap().contains_key(topic) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "{topic} was never cleared: {:?}",
                self.payloads(topic)
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// Every message `topic` had, in order; an emptied topic reads as null.
    pub fn seen(&self, topic: &str) -> Vec<Value> {
        self.history
            .lock()
            .unwrap()
            .iter()
            .filter(|(seen, _)| seen == topic)
            .map(|(_, bytes)| serde_json::from_slice(bytes).unwrap_or(Value::Null))
            .collect()
    }

    /// Waits until the broker has delivered every message published before Home
    /// Assistant announces itself, by announcing it and waiting for the server's
    /// answer: the discovery payload of `topic` once more.
    pub async fn settle(&self, topic: &str) {
        let before = self.seen(topic).len();
        self.publish("homeassistant/status", "online", false).await;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.seen(topic).len() <= before {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the server never answered"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// Waits until `topic` holds no message.
    pub async fn gone(&self, topic: &str) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.messages.lock().unwrap().contains_key(topic) {
            assert!(tokio::time::Instant::now() < deadline, "{topic} stayed");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    pub async fn publish(&self, topic: &str, payload: &str, retain: bool) {
        self.client
            .publish(
                topic,
                rumqttc::QoS::AtLeastOnce,
                retain,
                payload.as_bytes().to_vec(),
            )
            .await
            .unwrap();
    }
}
