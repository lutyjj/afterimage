//! One connection to a broker: the client, its event loop, and the publisher
//! that answers the broker and publishes each change. Retiring a connection
//! clears what it published and disconnects.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use afterimage_management_api::model::RefreshMinutes;
use rumqttc::tokio_rustls::rustls;
use rumqttc::{
    AsyncClient, ConnectionError, Event, LastWill, MqttOptions, Outgoing, Packet, QoS,
    TlsConfiguration, Transport,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::payloads::{
    PLAYLIST, device_key, discovery, firmware_state, key, mac_of, overdue, screen_options, state,
};
use super::{Change, Connection, Parts, Settings, Telemetry};
use crate::clock::Clock;
use crate::devices::{Changes, Device, Devices, Shows};
use crate::firmware::{Board, Firmware};
use crate::images::Images;
use crate::screens::Screens;

/// How long the connection waits after it failed before it tries again.
const RECONNECT: Duration = Duration::from_secs(5);
/// Requests the client queues for the connection.
const QUEUE: usize = 64;
/// How long a retiring connection waits for the broker to take its clearing and
/// its goodbye; past it, the connection ends all the same.
const RETIRE: Duration = Duration::from_secs(5);
/// How often the publisher checks whether a device has come to count as not
/// seen lately, which no check-in announces.
const OVERDUE_CHECK: Duration = Duration::from_secs(60);

/// The client's options for these settings, whose address
/// [`super::broker_url`] read: a host and a port, and TLS for `mqtts://`.
pub(super) fn options(settings: &Settings) -> Result<MqttOptions, String> {
    let url = &settings.url;
    let (Some(host), Some(port)) = (url.host_str(), url.port()) else {
        return Err("must name a host and a port".to_owned());
    };
    let mut options = MqttOptions::new(&settings.topic, host, port);
    // rumqttc's own TLS for `mqtts://` would panic: it asks rustls for a
    // process-wide crypto provider, which rustls cannot pick while the build
    // enables both of its providers, and it `expect`s the host's certificate
    // store to load without errors. So the transport is built here.
    if url.scheme() == "mqtts" {
        options.set_transport(tls_transport());
    }
    if let Some(username) = &settings.username {
        options.set_credentials(username, settings.password.as_deref().unwrap_or_default());
    }
    options.set_last_will(LastWill::new(
        format!("{}/status", settings.topic),
        "offline",
        QoS::AtLeastOnce,
        true,
    ));
    Ok(options)
}

/// What the publisher acts on besides the broker.
enum Message {
    Change(Change),
    /// Say goodbye and stop, clearing what it published first where `clear` is
    /// set; answered with whether the goodbye was sent.
    Retire {
        clear: bool,
        done: oneshot::Sender<bool>,
    },
}

/// An open connection: its tasks, and the way to hand them a change.
pub(super) struct Open {
    messages: mpsc::UnboundedSender<Message>,
    polling: JoinHandle<()>,
    publisher: JoinHandle<()>,
}

impl Open {
    /// Connects with `settings` and publishes from then on, reconnecting after any
    /// failure and keeping `state` current.
    pub(super) fn new(
        settings: Settings,
        parts: &Parts,
        telemetry: Telemetry,
        state: watch::Sender<Connection>,
    ) -> Result<Self, String> {
        let options = options(&settings)?;
        let status = format!("{}/status", settings.topic);
        let (client, mut eventloop) = AsyncClient::new(options, QUEUE);
        let (messages, mut pending) = mpsc::unbounded_channel();
        let (incoming_sender, mut incoming) = mpsc::unbounded_channel();
        state.send_replace(Connection::Connecting);

        // The event loop is polled on its own, since a publish waits on it. It
        // writes and flushes the goodbye before it reports it, then reads on until
        // the broker closes the connection, whatever has become of the publisher:
        // it holds a client of its own, and what the broker sends after the
        // publisher stopped listening (such as the echo of its own clearing) is
        // dropped rather than ending it. A socket closed with the broker's
        // messages unread is reset rather than closed, and the broker can then
        // lose the goodbye and publish the last will. Retiring aborts the loop if
        // the broker never closes.
        let holding = client.clone();
        let polling = tokio::spawn(async move {
            let _holding = holding;
            let mut connected = true;
            let mut leaving = false;
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        tracing::info!("connected to the MQTT broker");
                        connected = true;
                        state.send_replace(Connection::Connected);
                        let _ = incoming_sender.send(Incoming::Connected);
                    }
                    Ok(Event::Incoming(Packet::Publish(message))) => {
                        let message = Incoming::Message {
                            topic: message.topic,
                            payload: message.payload.to_vec(),
                            retained: message.retain,
                        };
                        let _ = incoming_sender.send(message);
                    }
                    Ok(Event::Outgoing(Outgoing::Disconnect)) => leaving = true,
                    Ok(_) => {}
                    Err(_) if leaving => return,
                    Err(error) => {
                        // A broker that answers with a refusal, such as for a wrong
                        // password, is told apart from one that cannot be reached.
                        let now = if matches!(error, ConnectionError::ConnectionRefused(_)) {
                            Connection::Refused
                        } else {
                            Connection::Connecting
                        };
                        state.send_if_modified(|state| std::mem::replace(state, now) != now);
                        if connected {
                            tracing::warn!(%error, "the MQTT broker cannot be reached or refused; trying again");
                            connected = false;
                            let _ = incoming_sender.send(Incoming::Disconnected);
                        }
                        tokio::time::sleep(RECONNECT).await;
                    }
                }
            }
        });

        let mut publisher = Publisher {
            client,
            settings,
            status,
            devices: Arc::clone(&parts.devices),
            screens: Arc::clone(&parts.screens),
            firmware: Arc::clone(&parts.firmware),
            images: Arc::clone(&parts.images),
            clock: Arc::clone(&parts.clock),
            telemetry,
            connected: false,
            published: HashMap::new(),
        };
        let publisher = tokio::spawn(async move {
            let mut overdue_check = tokio::time::interval(OVERDUE_CHECK);
            loop {
                // What the broker sent first, so a connection it has just
                // acknowledged counts as connected when a retirement follows.
                tokio::select! {
                    biased;
                    Some(event) = incoming.recv() => publisher.incoming(event).await,
                    Some(message) = pending.recv() => match message {
                        Message::Change(change) => publisher.change(change).await,
                        Message::Retire { clear, done } => {
                            let _ = done.send(publisher.retire(clear).await);
                            return;
                        }
                    },
                    _ = overdue_check.tick() => publisher.check_overdue().await,
                    else => return,
                }
            }
        });
        Ok(Self {
            messages,
            polling,
            publisher,
        })
    }

    pub(super) fn change(&self, change: Change) {
        // The publisher outlives every message but its retirement.
        let _ = self.messages.send(Message::Change(change));
    }

    /// Says goodbye, clearing what the connection published first where `clear`
    /// is set, and waits at most [`RETIRE`] for the broker; its tasks end either
    /// way. A broker out of reach is told nothing, so it keeps what was published.
    pub(super) async fn retire(self, clear: bool) {
        let (done, said) = oneshot::channel();
        let _ = self.messages.send(Message::Retire { clear, done });
        let mut polling = self.polling;
        let goodbye = tokio::time::timeout(RETIRE, async {
            if said.await.unwrap_or(false) {
                // The event loop ends once the broker has the goodbye.
                let _ = (&mut polling).await;
            }
        })
        .await;
        if goodbye.is_err() {
            tracing::warn!("the MQTT broker did not take the goodbye in time; leaving it");
        }
        polling.abort();
        self.publisher.abort();
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

/// What the publisher last published on a device's topics, which belong to a
/// MAC: the device that held them, its image, whether it counted as not seen
/// lately, and the "Showing" options Home Assistant was given, which a command
/// names.
struct Published {
    device: i64,
    image: Option<String>,
    overdue: bool,
    options: Vec<(String, i64)>,
}

/// A setting a command changes.
#[derive(Debug, PartialEq)]
enum Setting {
    Held(bool),
    /// An option of the "Showing" select.
    Showing(String),
    Refresh(i64),
}

struct Publisher {
    client: AsyncClient,
    settings: Settings,
    status: String,
    devices: Arc<Devices>,
    screens: Arc<Screens>,
    firmware: Arc<Firmware>,
    images: Arc<Images>,
    clock: Arc<dyn Clock>,
    /// Its own way in, for changes it causes.
    telemetry: Telemetry,
    connected: bool,
    /// By topic key, for this connection.
    published: HashMap<String, Published>,
}

impl Publisher {
    /// Says goodbye, first taking every device it published out of the broker
    /// and emptying its status where `clear` is set, or else marking its status
    /// offline, since the clean goodbye sends no last will and the next
    /// connection may be refused; `false` when it is not connected and so has
    /// nobody to tell.
    async fn retire(&mut self, clear: bool) -> bool {
        if !self.connected {
            tracing::info!("the MQTT broker is out of reach; what was published there stays");
            return false;
        }
        if clear {
            self.clear_all().await;
        } else {
            self.publish(&self.status.clone(), "offline").await;
        }
        if let Err(error) = self.client.disconnect().await {
            tracing::warn!(%error, "the MQTT goodbye was not sent");
            return false;
        }
        true
    }

    async fn clear_all(&mut self) {
        match self.devices.all().await {
            Ok(devices) => {
                for key in devices.iter().filter_map(|device| key(&device.mac)) {
                    self.clear(&key).await;
                }
            }
            Err(error) => {
                tracing::error!(%error, "the devices to take out of MQTT could not be read");
            }
        }
        self.publish(&self.status.clone(), "").await;
    }

    async fn incoming(&mut self, event: Incoming) {
        match event {
            Incoming::Connected => {
                self.connected = true;
                let subscriptions = [
                    format!("{}/status", self.settings.discovery_prefix),
                    format!("{}/+/firmware/install", self.settings.topic),
                    format!("{}/+/held/set", self.settings.topic),
                    format!("{}/+/showing/set", self.settings.topic),
                    format!("{}/+/refresh/set", self.settings.topic),
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
                } else if let Some((key, setting)) = setting(base, &topic, &payload)
                    && !retained
                {
                    self.set(key, setting);
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
            Change::Pressed(id) => match self.devices.any(id).await {
                Ok(Some(device)) => {
                    if let Some(key) = key(&device.mac) {
                        let topic = self.device_topic(&key, "button");
                        self.send(&topic, br#"{"event_type":"press"}"#.to_vec(), false)
                            .await;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::error!(%error, device = id, "a pressed device could not be read");
                }
            },
            Change::All => self.publish_all().await,
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
    async fn clear_if_gone(&mut self, key: &str) {
        let Some(mac) = mac_of(key) else {
            return;
        };
        if let Ok(None) = self.live(&mac).await {
            self.clear(key).await;
        }
    }

    /// Empties a device's retained topics, which takes it out of Home Assistant.
    async fn clear(&mut self, key: &str) {
        self.published.remove(key);
        for topic in [
            self.discovery_topic(key),
            self.device_topic(key, "state"),
            self.device_topic(key, "firmware"),
            self.device_topic(key, "image"),
        ] {
            self.publish(&topic, "").await;
        }
    }

    async fn publish_device(&mut self, device: &Device, boards: Option<&[Board]>) {
        let Some(key) = key(&device.mac) else {
            return;
        };
        let now = self.clock.now();
        let screen = self.screen_name(device).await;
        // Without its screens the select would offer and report the wrong
        // thing, so the device waits for its next change instead.
        let options = match self.screens.list(device.account_id).await {
            Ok(screens) => screen_options(&screens),
            Err(error) => {
                tracing::error!(%error, device = device.id, "the screens to offer could not be read");
                return;
            }
        };
        let config = discovery(&self.settings, &key, device, &options);
        self.publish(&self.discovery_topic(&key), &config.to_string())
            .await;
        self.publish(
            &self.device_topic(&key, "state"),
            &state(device, &screen, &options, now).to_string(),
        )
        .await;
        self.publish(
            &self.device_topic(&key, "firmware"),
            &firmware_state(device, boards).to_string(),
        )
        .await;
        let image = device.showing.as_ref().map(|showing| showing.image.clone());
        // The image topic is the MAC's, so what it carries may be another
        // device's that held the MAC. Images are named by their content.
        let before = self
            .published
            .get(&key)
            .and_then(|published| published.image.clone());
        if image.is_some() && image != before {
            self.publish_image(device.id, &key, image.as_deref()).await;
        }
        self.published.insert(
            key,
            Published {
                device: device.id,
                image,
                overdue: overdue(device, now),
                options,
            },
        );
    }

    async fn publish_image(&self, id: i64, key: &str, image: Option<&str>) {
        let Some(image) = image else {
            return;
        };
        match self.images.read(id, image) {
            Ok(Some(bytes)) => {
                self.send(&self.device_topic(key, "image"), bytes, true)
                    .await;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, device = id, "the image to publish could not be read");
            }
        }
    }

    /// Publishes each device whose not-seen-lately state has turned since it
    /// was last published, which no check-in announces.
    async fn check_overdue(&mut self) {
        if !self.connected {
            return;
        }
        let now = self.clock.now();
        let checked: Vec<(i64, bool)> = self
            .published
            .values()
            .map(|published| (published.device, published.overdue))
            .collect();
        for (id, was) in checked {
            let device = match self.devices.any(id).await {
                Ok(Some(device)) => device,
                Ok(None) => continue,
                Err(error) => {
                    tracing::error!(%error, device = id, "a device to check could not be read");
                    continue;
                }
            };
            // Only the device that holds its MAC's topics now speaks on them.
            let holds = matches!(self.live(&device.mac).await, Ok(Some(live)) if live.id == id);
            if holds && overdue(&device, now) != was {
                let boards = self.firmware.boards_on_hand();
                self.publish_device(&device, boards.as_deref()).await;
            }
        }
    }

    /// Applies a command to the device with this key, as the web UI would, in a
    /// task of its own, and publishes the device again. A "Showing" option is
    /// read against the options Home Assistant was given, so it names the screen
    /// it named there.
    fn set(&self, key: &str, setting: Setting) {
        let Some(mac) = mac_of(key) else {
            return;
        };
        let published = self.published.get(key);
        let changes = match setting {
            Setting::Held(held) => Changes {
                sleep_held: Some(held),
                ..Changes::default()
            },
            Setting::Refresh(minutes) => Changes {
                refresh_minutes: Some(minutes),
                ..Changes::default()
            },
            Setting::Showing(option) => {
                let shows = if option == PLAYLIST {
                    Some(Shows::Playlist)
                } else {
                    published.and_then(|published| {
                        published
                            .options
                            .iter()
                            .find(|(name, _)| *name == option)
                            .map(|(_, id)| Shows::Screen(*id))
                    })
                };
                let Some(shows) = shows else {
                    tracing::warn!(key, option, "no screen has that option");
                    // Home Assistant shows the option it sent until told.
                    if let Some(published) = published {
                        self.telemetry.device(published.device);
                    }
                    return;
                };
                Changes {
                    shows: Some(shows),
                    ..Changes::default()
                }
            }
        };
        let devices = Arc::clone(&self.devices);
        let telemetry = self.telemetry.clone();
        tokio::spawn(async move {
            let device = match devices.with_mac(&mac).await {
                Ok(Some(device)) => device,
                Ok(None) => return,
                Err(error) => {
                    tracing::error!(%error, "the device to set could not be read");
                    return;
                }
            };
            // A screen of another account, as when the MAC changed hands since
            // the options went out, is refused.
            if let Err(error) = devices.update(device.account_id, device.id, changes).await {
                tracing::warn!(%error, device = device.id, "a setting over MQTT was refused");
            }
            telemetry.device(device.id);
        });
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
            Some(showing) if showing.asleep => "Asleep".to_owned(),
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
        self.send(topic, payload.as_bytes().to_vec(), true).await;
    }

    async fn send(&self, topic: &str, payload: Vec<u8>, retain: bool) {
        let result = self
            .client
            .publish(topic, QoS::AtLeastOnce, retain, payload)
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

/// The device and the setting of a command topic, when its payload reads as
/// one: `ON` or `OFF` to hold sleep, an option to show, minutes between wakes.
fn setting<'a>(base: &str, topic: &'a str, payload: &[u8]) -> Option<(&'a str, Setting)> {
    let text = std::str::from_utf8(payload).ok()?.trim();
    if let Some(key) = device_key(base, topic, "held/set") {
        let held = match text {
            "ON" => true,
            "OFF" => false,
            _ => return None,
        };
        return Some((key, Setting::Held(held)));
    }
    if let Some(key) = device_key(base, topic, "showing/set") {
        return Some((key, Setting::Showing(text.to_owned())));
    }
    if let Some(key) = device_key(base, topic, "refresh/set") {
        // Home Assistant's number entity may send a whole number as `15.0`; the
        // model's own type decides the range.
        let minutes = text.parse::<f64>().ok()?;
        if minutes.fract() != 0.0 || !minutes.is_finite() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let minutes = RefreshMinutes::try_from(minutes as i32).ok()?;
        return Some((key, Setting::Refresh(i64::from(minutes.into_inner()))));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Setting, setting};

    fn parsed(leaf: &str, payload: &str) -> Option<Setting> {
        setting(
            "afterimage",
            &format!("afterimage/41b41039a101/{leaf}"),
            payload.as_bytes(),
        )
        .map(|(key, setting)| {
            assert_eq!(key, "41b41039a101");
            setting
        })
    }

    #[test]
    fn a_command_carries_a_setting_the_model_accepts() {
        assert_eq!(parsed("held/set", "ON"), Some(Setting::Held(true)));
        assert_eq!(parsed("held/set", "OFF"), Some(Setting::Held(false)));
        assert_eq!(parsed("refresh/set", "15"), Some(Setting::Refresh(15)));
        assert_eq!(parsed("refresh/set", "15.0"), Some(Setting::Refresh(15)));
        assert_eq!(parsed("refresh/set", "1440"), Some(Setting::Refresh(1440)));
        assert_eq!(
            parsed("showing/set", "Kitchen"),
            Some(Setting::Showing("Kitchen".to_owned()))
        );
    }

    #[test]
    fn a_malformed_command_carries_none() {
        for (leaf, payload) in [
            ("held/set", "on"),
            ("held/set", "maybe"),
            ("refresh/set", "0"),
            ("refresh/set", "1441"),
            ("refresh/set", "15.5"),
            ("refresh/set", "-15"),
            ("refresh/set", "NaN"),
            ("refresh/set", "inf"),
            ("refresh/set", "soon"),
            ("refresh/set", "99999999999"),
            ("firmware/set", "ON"),
        ] {
            assert_eq!(parsed(leaf, payload), None, "{leaf} {payload}");
        }
    }
}
