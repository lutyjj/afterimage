//! Settings, each from a command-line flag or its `AFTERIMAGE_*` environment
//! variable (`afterimage --help` lists them).

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::{oidc, telemetry};

#[derive(Debug, Clone, clap::Parser)]
#[command(
    name = "afterimage",
    version,
    about = "A self-hosted server for TRMNL e-paper displays."
)]
pub struct Config {
    /// Address to listen on; devices on the network must reach it.
    #[arg(long, env = "AFTERIMAGE_BIND", default_value = "0.0.0.0:8080")]
    pub bind: SocketAddr,

    /// Directory that holds the database.
    #[arg(long, env = "AFTERIMAGE_DATA_DIR", default_value = "data")]
    pub data_dir: PathBuf,

    /// Directory of files the server can fetch again, such as firmware images;
    /// `cache` in the data directory when unset.
    #[arg(long, env = "AFTERIMAGE_CACHE_DIR")]
    pub cache_dir: Option<PathBuf>,

    /// TRMNL's site, which publishes the firmware releases devices install.
    #[arg(long, env = "AFTERIMAGE_TRMNL_URL", default_value = "https://trmnl.com", value_parser = parse_origin)]
    pub trmnl_url: String,

    /// The address browsers reach the server at, such as `https://afterimage.example`.
    /// OIDC sign-in needs it, and an `https` address makes cookies `Secure`; without
    /// it, the reverse proxy's `X-Forwarded-Proto` decides.
    #[arg(long, env = "AFTERIMAGE_URL", value_parser = parse_origin)]
    pub public_url: Option<String>,

    /// Directory of the built web UI, served at `/`.
    #[arg(long, env = "AFTERIMAGE_WEB_DIR")]
    pub web_dir: Option<PathBuf>,

    /// The time zone of playlist times and of dates on screens, such as
    /// `Europe/Berlin`; the host's when unset, which then needs an IANA name.
    #[arg(long, env = "AFTERIMAGE_TIME_ZONE")]
    pub time_zone: Option<String>,

    /// Whether people may sign in with a password. Turning it off needs an OIDC
    /// provider.
    #[arg(long, env = "AFTERIMAGE_PASSWORD_SIGN_IN", default_value_t = true, action = clap::ArgAction::Set)]
    pub password_sign_in: bool,

    /// The OIDC provider's issuer URL. Its client must allow the redirect URL
    /// `<public URL>/manage/auth/oidc/callback`.
    #[arg(
        long,
        env = "AFTERIMAGE_OIDC_ISSUER",
        requires_all = ["oidc_client_id", "oidc_client_secret", "public_url"]
    )]
    pub oidc_issuer: Option<String>,

    #[arg(long, env = "AFTERIMAGE_OIDC_CLIENT_ID", requires = "oidc_issuer")]
    pub oidc_client_id: Option<String>,

    #[arg(
        long,
        env = "AFTERIMAGE_OIDC_CLIENT_SECRET",
        requires = "oidc_issuer",
        hide_env_values = true
    )]
    pub oidc_client_secret: Option<String>,

    /// What the sign-in button calls the OIDC provider.
    #[arg(long, env = "AFTERIMAGE_OIDC_NAME", default_value = "Single sign-on")]
    pub oidc_name: String,

    /// The MQTT broker to publish device state to, with Home Assistant's
    /// discovery, such as `mqtt://broker.example:1883` or `mqtts://` with TLS;
    /// nothing is published without it.
    #[arg(long, env = "AFTERIMAGE_MQTT_URL", value_parser = parse_mqtt_url)]
    pub mqtt_url: Option<url::Url>,

    #[arg(long, env = "AFTERIMAGE_MQTT_USERNAME", requires = "mqtt_url")]
    pub mqtt_username: Option<String>,

    #[arg(
        long,
        env = "AFTERIMAGE_MQTT_PASSWORD",
        requires = "mqtt_username",
        hide_env_values = true
    )]
    pub mqtt_password: Option<String>,

    /// The topic every device's state is published under, which is also the
    /// server's client ID at the broker.
    #[arg(long, env = "AFTERIMAGE_MQTT_TOPIC", default_value = "afterimage", value_parser = parse_topic)]
    pub mqtt_topic: String,

    /// Home Assistant's MQTT discovery prefix.
    #[arg(
        long,
        env = "AFTERIMAGE_MQTT_DISCOVERY_PREFIX",
        default_value = "homeassistant",
        value_parser = parse_topic
    )]
    pub mqtt_discovery_prefix: String,
}

impl Config {
    /// The OIDC provider, when one is configured.
    #[must_use]
    pub fn oidc(&self) -> Option<oidc::Settings> {
        Some(oidc::Settings {
            issuer: self.oidc_issuer.clone()?,
            client_id: self.oidc_client_id.clone()?,
            client_secret: self.oidc_client_secret.clone()?,
            name: self.oidc_name.clone(),
        })
    }

    /// Where the web UI lives: the public URL's root, or `/` on the same host.
    #[must_use]
    pub fn landing(&self) -> String {
        self.public_url.as_deref().map_or_else(
            || "/".to_owned(),
            |url| format!("{}/", url.trim_end_matches('/')),
        )
    }

    /// The broker and topics to publish to, when a broker is configured.
    #[must_use]
    pub fn mqtt(&self) -> Option<telemetry::Settings> {
        Some(telemetry::Settings {
            url: self.mqtt_url.clone()?,
            username: self.mqtt_username.clone(),
            password: self.mqtt_password.clone(),
            topic: self.mqtt_topic.clone(),
            discovery_prefix: self.mqtt_discovery_prefix.clone(),
            public_url: self.public_url.clone(),
        })
    }

    /// Where firmware images are cached.
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_dir
            .clone()
            .unwrap_or_else(|| self.data_dir.join("cache"))
    }

    /// Whether the public URL is `https`, when there is one.
    #[must_use]
    pub fn public_url_is_https(&self) -> Option<bool> {
        self.public_url
            .as_deref()
            .map(|url| url.starts_with("https://"))
    }
}

fn parse_mqtt_url(text: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(text).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "mqtt" | "mqtts") {
        return Err("must start with mqtt:// or mqtts://".to_owned());
    }
    if url.host_str().is_none_or(str::is_empty) || !matches!(url.path(), "" | "/") {
        return Err("must be a scheme, a host and a port, with no path".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("takes no credentials; set the MQTT username and password".to_owned());
    }
    Ok(url)
}

/// A topic of one or more levels, without wildcards or empty levels.
fn parse_topic(text: &str) -> Result<String, String> {
    let valid = !text.is_empty()
        && text.split('/').all(|level| {
            !level.is_empty()
                && !level.contains(['+', '#'])
                && level.chars().all(|c| c.is_ascii_graphic())
        });
    if valid {
        Ok(text.to_owned())
    } else {
        Err("must be topic levels of visible ASCII, without + or # or empty levels".to_owned())
    }
}

fn parse_origin(text: &str) -> Result<String, String> {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .ok_or("must start with https:// or http://")?;
    let host = rest.trim_end_matches('/');
    if host.is_empty() || host.contains(['/', '?', '#', '@']) {
        return Err("must be a scheme and host, with no path, query or fragment".to_owned());
    }
    Ok(text.trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    fn parse(arguments: &[&str]) -> Result<Config, clap::Error> {
        Config::try_parse_from(std::iter::once("afterimage").chain(arguments.iter().copied()))
    }

    #[test]
    fn the_command_is_well_formed() {
        Config::command().debug_assert();
    }

    #[test]
    fn defaults_to_every_interface_and_a_local_directory() {
        let config = parse(&[]).unwrap();
        assert_eq!(config.bind, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("data"));
        assert!(config.password_sign_in);
        assert!(config.oidc().is_none());
        assert_eq!(config.landing(), "/");
        assert_eq!(config.cache_dir(), PathBuf::from("data/cache"));
        assert_eq!(config.trmnl_url, "https://trmnl.com");
    }

    #[test]
    fn an_oidc_provider_needs_its_client_and_the_public_url() {
        assert!(parse(&["--oidc-issuer", "https://id.example.test"]).is_err());
        assert!(parse(&["--oidc-client-id", "afterimage"]).is_err());
        let config = parse(&[
            "--oidc-issuer",
            "https://id.example.test",
            "--oidc-client-id",
            "afterimage",
            "--oidc-client-secret",
            "secret",
            "--public-url",
            "https://afterimage.example/",
        ])
        .unwrap();
        assert_eq!(config.oidc().unwrap().client_id, "afterimage");
        assert_eq!(config.landing(), "https://afterimage.example/");
        assert_eq!(config.public_url_is_https(), Some(true));
    }

    #[test]
    fn mqtt_needs_a_broker_url_without_credentials_and_clean_topics() {
        assert!(parse(&[]).unwrap().mqtt().is_none());
        let config = parse(&["--mqtt-url", "mqtt://broker.example.test:1883"]).unwrap();
        let mqtt = config.mqtt().unwrap();
        assert_eq!(
            (mqtt.topic.as_str(), mqtt.discovery_prefix.as_str()),
            ("afterimage", "homeassistant")
        );
        for url in [
            "http://broker.example.test",
            "mqtt://",
            "mqtt://broker.example.test/path",
            "mqtt://user:secret@broker.example.test",
        ] {
            assert!(parse(&["--mqtt-url", url]).is_err(), "{url}");
        }
        assert!(
            parse(&["--mqtt-username", "afterimage"]).is_err(),
            "a username needs a broker"
        );
        for topic in ["", "a/#", "a/+/b", "a//b", "a b"] {
            assert!(parse(&["--mqtt-topic", topic]).is_err(), "{topic:?}");
        }
        assert!(parse(&["--mqtt-topic", "home/afterimage"]).is_ok());
    }

    #[test]
    fn refuses_a_public_url_that_is_not_an_origin() {
        for url in [
            "afterimage.example",
            "https://",
            "https://afterimage.example/?a=b",
            "https://afterimage.example/sub",
        ] {
            assert!(parse(&["--public-url", url]).is_err(), "{url}");
        }
    }
}
