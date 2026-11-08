//! Device state over MQTT, with Home Assistant's MQTT discovery. Each claimed
//! device becomes a Home Assistant device: its battery, voltage, signal, last
//! check-in, current screen and next wake are sensors; whether it sleeps and
//! whether it counts as not seen lately are binary sensors; its firmware is an
//! update entity whose install asks the device to install its board's newest
//! release (step 4 of the design's Devices); a switch holds it asleep, a select
//! pins one of its account's playlists or screens in place of its schedule, a
//! number sets how often it wakes, a press on its button is an event, and the
//! image it shows is an image entity.
//!
//! Topics, under the base topic (`afterimage` unless configured), where `<device>`
//! is the device's MAC in lowercase hexadecimal:
//!
//! - `<base>/status`: `online` while the server is connected, and `offline`, its
//!   last will. Every entity's availability follows it.
//! - `<base>/<device>/state`: the readings, the screen, sleep, wake and the
//!   settings the controls show, as JSON.
//! - `<base>/<device>/firmware`: the installed and newest release, as Home
//!   Assistant's update entity reads them.
//! - `<base>/<device>/image`: the image the device shows, as PNG.
//! - `<base>/<device>/button`: `{"event_type": "press"}` when its button woke it.
//! - Commands: `<base>/<device>/firmware/install` (`install`),
//!   `<base>/<device>/held/set` (`ON` or `OFF`), `<base>/<device>/showing/set`
//!   (an option of the select) and `<base>/<device>/refresh/set` (minutes).
//! - `<prefix>/device/<base>_<device>/config`: the device's discovery payload.
//!
//! Everything but the commands and the button is retained, so a subscriber that
//! joins later reads the last state. The server publishes every device again
//! when it connects, when Home Assistant announces itself on
//! `<prefix>/status`, and when an account's playlists or screens change, since
//! they are the select's options; one device whenever it checks in or changes; and a device
//! whose not-seen-lately state turns, checked every minute. On connecting it
//! also reads the retained state topics back and clears those of devices that
//! are gone, so a device removed while the broker was out of reach does not
//! linger. History belongs to whoever subscribes.
//!
//! Accounts may hold devices of one MAC, as when a device set up again is claimed
//! by another account; its topics belong to the one that checked in last.
//!
//! Whoever may publish to the broker may command any device: the broker's
//! access control is the boundary. A retained command is ignored, since the
//! broker would replay it at every connection.
//!
//! The broker is the deployment's, or else the one the administrator saved in
//! the web UI ([`Saved`]). A change of broker or topics takes effect at once: the
//! connection that ends clears every topic it published and says goodbye, so its
//! last will stays unsent, and the next one connects with the new settings.

mod connection;
mod payloads;
mod saved;

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, watch};

pub use saved::{Saved, SavedBroker};

use crate::clock::Clock;
use crate::devices::Devices;
use crate::firmware::Firmware;
use crate::images::Images;
use crate::playlists::Playlists;
use crate::screens::Screens;

/// Where and how to publish.
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Settings {
    /// Whether the client takes these settings.
    ///
    /// # Errors
    ///
    /// What it refuses, such as a URL it cannot connect to.
    pub fn check(&self) -> Result<(), String> {
        connection::options(self).map(drop)
    }
}

/// A broker's address: `mqtt://` or `mqtts://`, a host and a port, without a
/// path or credentials, which the username and password carry, or a query,
/// which the MQTT client would read as its own options.
///
/// # Errors
///
/// What the address lacks or has too much of.
pub fn broker_url(text: &str) -> Result<url::Url, String> {
    let mut url = url::Url::parse(text).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "mqtt" | "mqtts") {
        return Err("must start with mqtt:// or mqtts://".to_owned());
    }
    if url.host_str().is_none_or(str::is_empty) || !matches!(url.path(), "" | "/") {
        return Err("must be a scheme, a host and a port, with no path".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("takes no credentials; set the MQTT username and password".to_owned());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("takes no query or fragment".to_owned());
    }
    // One spelling for one broker: no closing slash, and the port the client
    // connects to when none is given (rumqttc's 1883, or 8883 with TLS).
    url.set_path("");
    if url.port().is_none() {
        let port = if url.scheme() == "mqtts" { 8883 } else { 1883 };
        url.set_port(Some(port))
            .map_err(|()| "must name a host".to_owned())?;
    }
    Ok(url)
}

/// Whether two addresses name one broker: the same scheme, host and port.
#[must_use]
pub fn same_broker(one: &url::Url, other: &url::Url) -> bool {
    (one.scheme(), one.host_str(), one.port()) == (other.scheme(), other.host_str(), other.port())
}

/// The connection to the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    /// No broker is set.
    Off,
    /// Connecting, or trying again after the broker could not be reached.
    Connecting,
    /// The broker refused the connection, such as for a wrong username or
    /// password; trying again.
    Refused,
    Connected,
}

/// What changed, for the publisher.
#[derive(Debug)]
enum Change {
    Device(i64),
    /// A removed device, by its MAC.
    Removed(String),
    /// A press on the device's button woke it.
    Pressed(i64),
    /// Every device, such as when playlists or screens, the select's options,
    /// change.
    All,
}

enum Command {
    Change(Change),
    /// New settings, or none to stop publishing; answered once they hold.
    Configure(Option<Settings>, oneshot::Sender<Result<(), String>>),
}

/// What every connection publishes from.
struct Parts {
    devices: Arc<Devices>,
    screens: Arc<Screens>,
    playlists: Arc<Playlists>,
    firmware: Arc<Firmware>,
    images: Arc<Images>,
    clock: Arc<dyn Clock>,
}

/// The way in for everything that changes a device, and for the broker's
/// settings. One task owns the connection and hands each change to it; with no
/// broker it drops them.
#[derive(Debug, Clone)]
pub struct Telemetry {
    commands: mpsc::UnboundedSender<Command>,
    connection: watch::Receiver<Connection>,
}

impl Telemetry {
    /// Starts publishing with `settings`, connecting to the broker and
    /// reconnecting after any failure, or waits for settings when there are none.
    ///
    /// # Errors
    ///
    /// When the client does not take the settings.
    pub fn start(
        settings: Option<Settings>,
        devices: Devices,
        screens: Screens,
        playlists: Playlists,
        firmware: Arc<Firmware>,
        images: Arc<Images>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let parts = Parts {
            devices: Arc::new(devices),
            screens: Arc::new(screens),
            playlists: Arc::new(playlists),
            firmware,
            images,
            clock,
        };
        let (state, connection) = watch::channel(Connection::Off);
        let (commands, mut received) = mpsc::unbounded_channel();
        let telemetry = Self {
            commands,
            connection,
        };
        let open = |settings: Settings| {
            connection::Open::new(settings, &parts, telemetry.clone(), state.clone())
        };
        let mut current = settings
            .map(|settings| open(settings.clone()).map(|opened| (settings, opened)))
            .transpose()?;
        let telemetry_of_task = telemetry.clone();
        tokio::spawn(async move {
            let open = |settings: Settings| {
                connection::Open::new(settings, &parts, telemetry_of_task.clone(), state.clone())
            };
            while let Some(command) = received.recv().await {
                match command {
                    Command::Change(change) => {
                        if let Some((_, current)) = &current {
                            current.change(change);
                        }
                    }
                    Command::Configure(settings, done) => {
                        // Nothing changes for the same settings.
                        if current.as_ref().map(|(held, _)| held) == settings.as_ref() {
                            let _ = done.send(Ok(()));
                            continue;
                        }
                        if let Some((held, retiring)) = current.take() {
                            // What was published stays where only the credentials
                            // change; another broker or topics take it away.
                            let moves = settings.as_ref().is_none_or(|next| {
                                !same_broker(&next.url, &held.url)
                                    || (&next.topic, &next.discovery_prefix)
                                        != (&held.topic, &held.discovery_prefix)
                            });
                            retiring.retire(moves).await;
                        }
                        state.send_replace(Connection::Off);
                        let opened = settings
                            .map(|settings| open(settings.clone()).map(|opened| (settings, opened)))
                            .transpose();
                        let answer = opened.map(|opened| current = opened);
                        // The caller may have stopped waiting; the settings hold
                        // all the same.
                        let _ = done.send(answer);
                    }
                }
            }
        });
        Ok(telemetry)
    }

    /// Publishes the device's discovery payload and state.
    pub fn device(&self, id: i64) {
        self.send(Command::Change(Change::Device(id)));
    }

    /// Takes a removed device out of Home Assistant.
    pub fn removed(&self, mac: &str) {
        self.send(Command::Change(Change::Removed(mac.to_owned())));
    }

    /// Tells Home Assistant that a press on the device's button woke it.
    pub fn pressed(&self, id: i64) {
        self.send(Command::Change(Change::Pressed(id)));
    }

    /// Publishes every device again, after an account's playlists or screens,
    /// which the select offers, changed.
    pub fn options_changed(&self) {
        self.send(Command::Change(Change::All));
    }

    /// The connection to the broker now.
    #[must_use]
    pub fn connection(&self) -> Connection {
        *self.connection.borrow()
    }

    /// Publishes with `settings` from now on, or stops publishing with none.
    /// Another broker or other topics first clear what the current connection
    /// published, when its broker can be reached; other credentials reconnect
    /// and keep it; the same settings change nothing.
    ///
    /// # Errors
    ///
    /// When the client does not take the settings, which leaves the server
    /// publishing nowhere.
    pub async fn configure(&self, settings: Option<Settings>) -> Result<(), String> {
        let (done, answer) = oneshot::channel();
        self.send(Command::Configure(settings, done));
        answer
            .await
            .unwrap_or_else(|_| Err("the publisher has stopped".to_owned()))
    }

    fn send(&self, command: Command) {
        // The task outlives every sender but at shutdown.
        let _ = self.commands.send(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_broker_has_one_address_however_it_is_spelt() {
        let same = |one: &str, other: &str| {
            same_broker(&broker_url(one).unwrap(), &broker_url(other).unwrap())
        };
        assert!(same(
            "mqtt://broker.example.test",
            "mqtt://broker.example.test:1883/"
        ));
        assert!(same(
            "mqtts://broker.example.test",
            "mqtts://broker.example.test:8883"
        ));
        assert!(!same(
            "mqtt://broker.example.test",
            "mqtts://broker.example.test"
        ));
        assert!(!same(
            "mqtt://broker.example.test",
            "mqtt://broker.example.test:1884"
        ));
        assert_eq!(
            broker_url("mqtt://broker.example.test/").unwrap().as_str(),
            "mqtt://broker.example.test:1883"
        );
    }
}
