//! One connection to a broker: the client, its event loop, and the publisher
//! that answers the broker and publishes each change. Retiring a connection
//! clears what it published and disconnects.

use std::sync::Arc;
use std::time::Duration;

use rumqttc::tokio_rustls::rustls;
use rumqttc::{
    AsyncClient, Event, LastWill, MqttOptions, Outgoing, Packet, QoS, TlsConfiguration, Transport,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::payloads::{device_key, discovery, firmware_state, key, mac_of, state};
use super::{Change, Connection, Parts, Settings, Telemetry};
use crate::devices::{Device, Devices};
use crate::firmware::{Board, Firmware};
use crate::screens::Screens;

/// How long the connection waits after it failed before it tries again.
const RECONNECT: Duration = Duration::from_secs(5);
/// Requests the client queues for the connection.
const QUEUE: usize = 64;
/// How long a retiring connection waits for the broker to take its clearing and
/// its goodbye; past it, the connection ends all the same.
const RETIRE: Duration = Duration::from_secs(5);

/// The client's options for these settings.
pub(super) fn options(settings: &Settings) -> Result<MqttOptions, String> {
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
    /// Clear, say goodbye and stop; answered with whether the goodbye was sent.
    Retire(oneshot::Sender<bool>),
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

        // The event loop is polled on its own, since a publish waits on it. After
        // the goodbye it goes on polling, which sends it, until the broker closes
        // the connection: a connection dropped before the goodbye reaches the
        // broker would have the broker publish the last will.
        let polling = tokio::spawn(async move {
            let mut connected = true;
            let mut leaving = false;
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        tracing::info!("connected to the MQTT broker");
                        connected = true;
                        state.send_replace(Connection::Connected);
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
                    Ok(Event::Outgoing(Outgoing::Disconnect)) => leaving = true,
                    Ok(_) => {}
                    Err(_) if leaving => return,
                    Err(error) => {
                        if connected {
                            tracing::warn!(%error, "the MQTT broker cannot be reached; trying again");
                            connected = false;
                            state.send_replace(Connection::Connecting);
                            if incoming_sender.send(Incoming::Disconnected).is_err() {
                                return;
                            }
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
            telemetry,
            connected: false,
        };
        let publisher = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = incoming.recv() => publisher.incoming(event).await,
                    Some(message) = pending.recv() => match message {
                        Message::Change(change) => publisher.change(change).await,
                        Message::Retire(done) => {
                            let _ = done.send(publisher.retire().await);
                            return;
                        }
                    },
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

    /// Clears what the connection published and says goodbye, waiting at most
    /// [`RETIRE`] for the broker; its tasks end either way.
    pub(super) async fn retire(self) {
        let (done, said) = oneshot::channel();
        let _ = self.messages.send(Message::Retire(done));
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

struct Publisher {
    client: AsyncClient,
    settings: Settings,
    status: String,
    devices: Arc<Devices>,
    screens: Arc<Screens>,
    firmware: Arc<Firmware>,
    /// Its own way in, for changes it causes.
    telemetry: Telemetry,
    connected: bool,
}

impl Publisher {
    /// Takes every device it published out of the broker, empties its status, and
    /// says goodbye; `false` when it is not connected and so has nobody to tell.
    async fn retire(&mut self) -> bool {
        if !self.connected {
            return false;
        }
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
        if let Err(error) = self.client.disconnect().await {
            tracing::warn!(%error, "the MQTT goodbye was not sent");
            return false;
        }
        true
    }

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
