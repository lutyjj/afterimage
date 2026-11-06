//! Device state over MQTT, with Home Assistant's MQTT discovery. Each claimed
//! device becomes a Home Assistant device: its battery, voltage, signal, last
//! check-in and current screen are sensors, and its firmware is an update entity
//! whose install asks the device to install its board's newest release (step 4 of
//! the design's Devices).
//!
//! Topics, under the base topic (`afterimage` unless configured), where `<device>`
//! is the device's MAC in lowercase hexadecimal:
//!
//! - `<base>/status`: `online` while the server is connected, and `offline`, its
//!   last will. Every entity's availability follows it.
//! - `<base>/<device>/state`: the readings and the screen, as JSON.
//! - `<base>/<device>/firmware`: the installed and newest release, as Home
//!   Assistant's update entity reads them.
//! - `<base>/<device>/firmware/install`: `install` asks for the newest release.
//! - `<prefix>/device/<base>_<device>/config`: the device's discovery payload.
//!
//! Everything but the install command is retained, so a subscriber that joins
//! later reads the last state. The server publishes every device again when it
//! connects and when Home Assistant announces itself on `<prefix>/status`, and
//! one device whenever it checks in or changes. On connecting it also reads the
//! retained state topics back and clears those of devices that are gone, so a
//! device removed while the broker was out of reach does not linger. History
//! belongs to whoever subscribes.
//!
//! Accounts may hold devices of one MAC, as when a device set up again is claimed
//! by another account; its topics belong to the one that checked in last.
//!
//! Whoever may publish to the broker may ask any device to install firmware: the
//! broker's access control is the boundary. A retained install command is
//! ignored, since the broker would replay it at every connection.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use afterimage_trmnl::Mac;
use rumqttc::tokio_rustls::rustls;
use rumqttc::{
    AsyncClient, Event, LastWill, MqttOptions, Packet, QoS, TlsConfiguration, Transport,
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::devices::{Device, Devices};
use crate::firmware::{self, Board, Firmware};
use crate::screens::Screens;

/// How long the connection waits after it failed before it tries again.
const RECONNECT: Duration = Duration::from_secs(5);
/// Requests the client queues for the connection.
const QUEUE: usize = 64;

/// Where and how to publish.
#[derive(Debug, Clone)]
pub struct Settings {
    /// `mqtt://` or `mqtts://`, host and port.
    pub url: url::Url,
    pub username: Option<String>,
    pub password: Option<String>,
    /// The base topic, which is also the client's ID at the broker.
    pub topic: String,
    /// Home Assistant's discovery prefix.
    pub discovery_prefix: String,
    /// The web UI's address, for each device's link back to its page.
    pub public_url: Option<String>,
}

/// What changed, for the publisher.
#[derive(Debug)]
enum Change {
    Device(i64),
    /// A removed device, by its MAC.
    Removed(String),
}

/// The way in for everything that changes a device. Without a broker it drops
/// every change.
#[derive(Debug, Clone, Default)]
pub struct Telemetry {
    changes: Option<mpsc::UnboundedSender<Change>>,
}

impl Telemetry {
    /// Publishes the device's discovery payload and state.
    pub fn device(&self, id: i64) {
        self.send(Change::Device(id));
    }

    /// Takes a removed device out of Home Assistant.
    pub fn removed(&self, mac: &str) {
        self.send(Change::Removed(mac.to_owned()));
    }

    fn send(&self, change: Change) {
        if let Some(changes) = &self.changes {
            // The publisher outlives every sender but at shutdown.
            let _ = changes.send(change);
        }
    }

    /// Connects to the broker and publishes from then on, reconnecting after
    /// any failure.
    ///
    /// # Errors
    ///
    /// When the URL is not one the client takes.
    pub fn start(
        settings: Settings,
        devices: Devices,
        screens: Screens,
        firmware: Arc<Firmware>,
    ) -> Result<Self, String> {
        let mut url = settings.url.clone();
        // rumqttc's own TLS for `mqtts://` panics here: it asks rustls for a
        // process-wide crypto provider, which rustls cannot pick while the build
        // enables both of its providers, and it `expect`s the host's certificate
        // store to load without errors. So the URL is read as plain `mqtt://` and
        // the TLS transport set here.
        let tls = url.scheme() == "mqtts";
        if tls {
            if url.port().is_none() {
                url.set_port(Some(8883))
                    .map_err(|()| "an mqtts:// URL without a host".to_owned())?;
            }
            url.set_scheme("mqtt")
                .map_err(|()| "an mqtts:// URL that cannot be read as mqtt://".to_owned())?;
        }
        if !url.query_pairs().any(|(name, _)| name == "client_id") {
            url.query_pairs_mut()
                .append_pair("client_id", &settings.topic);
        }
        let mut options = MqttOptions::try_from(url).map_err(|error| error.to_string())?;
        if tls {
            options.set_transport(tls_transport());
        }
        if let Some(username) = &settings.username {
            options.set_credentials(username, settings.password.as_deref().unwrap_or_default());
        }
        let status = format!("{}/status", settings.topic);
        options.set_last_will(LastWill::new(&status, "offline", QoS::AtLeastOnce, true));
        let (client, mut eventloop) = AsyncClient::new(options, QUEUE);
        let (changes, mut pending) = mpsc::unbounded_channel();
        let (incoming_sender, mut incoming) = mpsc::unbounded_channel();

        // The event loop is polled on its own, since a publish waits on it.
        tokio::spawn(async move {
            let mut connected = true;
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        tracing::info!("connected to the MQTT broker");
                        connected = true;
                        if incoming_sender.send(Incoming::Connected).is_err() {
                            return;
                        }
                    }
                    Ok(Event::Incoming(Packet::Publish(message))) => {
                        let message = Incoming::Message {
                            topic: message.topic,
                            payload: message.payload.to_vec(),
                            retained: message.retain,
                        };
                        if incoming_sender.send(message).is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if connected {
                            tracing::warn!(%error, "the MQTT broker cannot be reached; trying again");
                            connected = false;
                            if incoming_sender.send(Incoming::Disconnected).is_err() {
                                return;
                            }
                        }
                        tokio::time::sleep(RECONNECT).await;
                    }
                }
            }
        });

        let telemetry = Self {
            changes: Some(changes),
        };
        let publisher = Publisher {
            client,
            settings,
            status,
            devices: Arc::new(devices),
            screens,
            firmware,
            telemetry: telemetry.clone(),
            connected: false,
        };
        tokio::spawn(async move {
            let mut publisher = publisher;
            loop {
                tokio::select! {
                    Some(event) = incoming.recv() => publisher.incoming(event).await,
                    Some(change) = pending.recv() => publisher.change(change).await,
                    else => return,
                }
            }
        });
        Ok(telemetry)
    }
}

/// TLS for an `mqtts://` broker: rustls with its aws-lc-rs provider and the
/// host's certificate store, or an empty store when the host has none, as
/// [`crate::outbound::Outbound`] does, so that such a host still serves devices.
fn tls_transport() -> Transport {
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for error in &native.errors {
        tracing::warn!(%error, "a host CA certificate could not be read for MQTT over TLS");
    }
    let (added, _) = roots.add_parsable_certificates(native.certs);
    if added == 0 {
        tracing::warn!(
            "MQTT over TLS will fail: the host has no CA certificates; install them and restart"
        );
    }
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("aws-lc-rs supports rustls's default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    Transport::tls_with_config(TlsConfiguration::Rustls(Arc::new(config)))
}

/// What the broker sent.
enum Incoming {
    Connected,
    Disconnected,
    Message {
        topic: String,
        payload: Vec<u8>,
        /// Delivered from the broker's retained store rather than published now.
        retained: bool,
    },
}

struct Publisher {
    client: AsyncClient,
    settings: Settings,
    status: String,
    devices: Arc<Devices>,
    screens: Screens,
    firmware: Arc<Firmware>,
    /// Its own way in, for changes it causes.
    telemetry: Telemetry,
    connected: bool,
}

impl Publisher {
    async fn incoming(&mut self, event: Incoming) {
        match event {
            Incoming::Connected => {
                self.connected = true;
                let subscriptions = [
                    format!("{}/status", self.settings.discovery_prefix),
                    format!("{}/+/firmware/install", self.settings.topic),
                    // The retained ones arrive on subscribing, which clears what
                    // devices since removed left. The subscription stays, and the
                    // server's own state comes back to it unflagged and is ignored.
                    format!("{}/+/state", self.settings.topic),
                ];
                for topic in subscriptions {
                    if let Err(error) = self.client.subscribe(topic, QoS::AtLeastOnce).await {
                        tracing::warn!(%error, "an MQTT subscription failed");
                    }
                }
                self.publish(&self.status.clone(), "online").await;
                self.publish_all().await;
            }
            Incoming::Disconnected => self.connected = false,
            Incoming::Message {
                topic,
                payload,
                retained,
            } => {
                let base = &self.settings.topic;
                if topic == format!("{}/status", self.settings.discovery_prefix) {
                    if payload == b"online" {
                        self.publish_all().await;
                    }
                } else if let Some(key) = device_key(base, &topic, "firmware/install") {
                    if payload == b"install" && !retained {
                        self.install(key);
                    }
                } else if let Some(key) = device_key(base, &topic, "state")
                    && retained
                    && !payload.is_empty()
                {
                    self.clear_if_gone(key).await;
                }
            }
        }
    }

    async fn change(&mut self, change: Change) {
        // A reconnect publishes everything, so nothing waits while disconnected.
        if !self.connected {
            return;
        }
        match change {
            Change::Device(id) => match self.devices.any(id).await {
                Ok(Some(device)) => {
                    // A device whose MAC another device holds more recently
                    // leaves the topics to that one.
                    if let Ok(Some(live)) = self.live(&device.mac).await
                        && live.id == device.id
                    {
                        let boards = self.firmware.boards_on_hand();
                        self.publish_device(&device, boards.as_deref()).await;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::error!(%error, device = id, "a device to publish could not be read");
                }
            },
            Change::Removed(mac) => match self.live(&mac).await {
                Ok(Some(other)) => {
                    let boards = self.firmware.boards_on_hand();
                    self.publish_device(&other, boards.as_deref()).await;
                }
                Ok(None) => {
                    if let Some(key) = key(&mac) {
                        self.clear(&key).await;
                    }
                }
                // Unknown is not gone: the topics stay until the next change.
                Err(()) => {}
            },
        }
    }

    async fn publish_all(&mut self) {
        let devices = match self.devices.all().await {
            Ok(devices) => devices,
            Err(error) => {
                tracing::error!(%error, "the devices to publish could not be read");
                return;
            }
        };
        let boards = self.firmware.boards_on_hand();
        for device in &devices {
            if let Ok(Some(live)) = self.live(&device.mac).await
                && live.id == device.id
            {
                self.publish_device(device, boards.as_deref()).await;
            }
        }
    }

    /// The device whose MAC this is and which checked in last, when any holds
    /// it; `Err` when storage could not tell, which is logged.
    async fn live(&self, mac: &str) -> Result<Option<Device>, ()> {
        self.devices.with_mac(mac).await.map_err(|error| {
            tracing::error!(%error, "a device to publish could not be read");
        })
    }

    /// Clears a key's topics when no device holds its MAC any more.
    async fn clear_if_gone(&self, key: &str) {
        let Some(mac) = mac_of(key) else {
            return;
        };
        if let Ok(None) = self.live(&mac).await {
            self.clear(key).await;
        }
    }

    /// Empties a device's retained topics, which takes it out of Home Assistant.
    async fn clear(&self, key: &str) {
        for topic in [
            self.discovery_topic(key),
            self.device_topic(key, "state"),
            self.device_topic(key, "firmware"),
        ] {
            self.publish(&topic, "").await;
        }
    }

    async fn publish_device(&self, device: &Device, boards: Option<&[Board]>) {
        let Some(key) = key(&device.mac) else {
            return;
        };
        let screen = self.screen_name(device).await;
        let config = discovery(&self.settings, &key, device);
        self.publish(&self.discovery_topic(&key), &config.to_string())
            .await;
        self.publish(
            &self.device_topic(&key, "state"),
            &state(device, &screen).to_string(),
        )
        .await;
        self.publish(
            &self.device_topic(&key, "firmware"),
            &firmware_state(device, boards).to_string(),
        )
        .await;
    }

    /// Asks the device with this key for its board's newest release, as the web
    /// UI's install does, in a task of its own: the install downloads the release
    /// first, and publishing goes on meanwhile.
    fn install(&self, key: &str) {
        let Some(mac) = mac_of(key) else {
            return;
        };
        let devices = Arc::clone(&self.devices);
        let firmware = Arc::clone(&self.firmware);
        let telemetry = self.telemetry.clone();
        tokio::spawn(async move {
            let device = match devices.with_mac(&mac).await {
                Ok(Some(device)) => device,
                Ok(None) => return,
                Err(error) => {
                    tracing::error!(%error, "the device to update could not be read");
                    return;
                }
            };
            match firmware.install(&device, None).await {
                Ok(()) => {
                    tracing::info!(device = device.id, "firmware install requested over MQTT");
                }
                Err(error) => {
                    tracing::warn!(%error, device = device.id, "firmware install over MQTT refused");
                }
            }
            telemetry.device(device.id);
        });
    }

    async fn screen_name(&self, device: &Device) -> String {
        match &device.showing {
            None => "Nothing yet".to_owned(),
            Some(showing) => match showing.screen_id {
                None => "Afterimage's own screen".to_owned(),
                Some(id) => match self.screens.get(device.account_id, id).await {
                    Ok(Some(screen)) => screen.name,
                    _ => "A screen".to_owned(),
                },
            },
        }
    }

    async fn publish(&self, topic: &str, payload: &str) {
        let result = self
            .client
            .publish(topic, QoS::AtLeastOnce, true, payload.as_bytes().to_vec())
            .await;
        if let Err(error) = result {
            tracing::warn!(%error, topic, "an MQTT message was not sent");
        }
    }

    fn device_topic(&self, key: &str, leaf: &str) -> String {
        format!("{}/{key}/{leaf}", self.settings.topic)
    }

    fn discovery_topic(&self, key: &str) -> String {
        format!(
            "{}/device/{}_{key}/config",
            self.settings.discovery_prefix, self.settings.topic
        )
    }
}

/// A device's key in topics: its MAC in lowercase hexadecimal.
fn key(mac: &str) -> Option<String> {
    let mac = Mac::from_str(mac).ok()?.to_string();
    Some(mac.replace(':', "").to_lowercase())
}

/// The MAC, as the registrations store it, of a key from a topic.
fn mac_of(key: &str) -> Option<String> {
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
fn device_key<'a>(base: &str, topic: &'a str, leaf: &str) -> Option<&'a str> {
    topic
        .strip_prefix(base)?
        .strip_prefix('/')?
        .strip_suffix(leaf)?
        .strip_suffix('/')
        .filter(|key| !key.is_empty() && !key.contains('/'))
}

/// The device's readings and screen, as its sensors read them.
fn state(device: &Device, screen: &str) -> Value {
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

/// The device's firmware, in the shape of Home Assistant's update entity. An
/// install request between its request and the check-in that decides it is in
/// progress. A version the server does not know is left out: the entity refuses
/// a null one, which it reads as a string.
fn firmware_state(device: &Device, boards: Option<&[Board]>) -> Value {
    let latest = boards
        .and_then(|boards| Firmware::board(boards, device))
        .and_then(Board::latest)
        .map(|release| release.version.clone());
    let in_progress = device.firmware_update.as_ref().is_some_and(|update| {
        matches!(
            update.state(),
            firmware::State::Requested | firmware::State::Sent
        )
    });
    let mut state = json!({ "in_progress": in_progress, "title": "TRMNL firmware" });
    if let Some(state) = state.as_object_mut() {
        if let Some(installed) = &device.readings.firmware_version {
            state.insert("installed_version".to_owned(), json!(installed));
        }
        if let Some(latest) = latest {
            state.insert("latest_version".to_owned(), json!(latest));
        }
    }
    state
}

/// The device's discovery payload: the device, the server as its origin, and
/// one component per entity.
fn discovery(settings: &Settings, key: &str, device: &Device) -> Value {
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
    // Home Assistant drops a device whose optional fields are null, so an
    // unknown one is left out.
    let mut device_info = json!({
        "identifiers": [format!("{base}_{key}")],
        "connections": [["mac", device.mac.to_lowercase()]],
        "name": device.name,
    });
    if let Some(info) = device_info.as_object_mut() {
        if !device.firmware_model.is_empty() {
            info.insert("model".to_owned(), json!(device.firmware_model));
        }
        if let Some(version) = &device.readings.firmware_version {
            info.insert("sw_version".to_owned(), json!(version));
        }
        if let Some(url) = &settings.public_url {
            info.insert(
                "configuration_url".to_owned(),
                json!(format!("{url}/devices/{}", device.id)),
            );
        }
    }
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
        assert_eq!(device_key("afterimage", "afterimage/a/firmware", "state"), None);
    }
}
