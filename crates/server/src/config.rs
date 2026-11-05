//! Settings, each from a command-line flag or its `AFTERIMAGE_*` environment
//! variable (`afterimage --help` lists them).

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::oidc;

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

    /// The address browsers reach the server at, such as `https://afterimage.example`.
    /// OIDC sign-in needs it, and an `https` address makes cookies `Secure`; without
    /// it, the reverse proxy's `X-Forwarded-Proto` decides.
    #[arg(long, env = "AFTERIMAGE_URL", value_parser = parse_public_url)]
    pub public_url: Option<String>,

    /// Directory of the built web UI, served at `/`.
    #[arg(long, env = "AFTERIMAGE_WEB_DIR")]
    pub web_dir: Option<PathBuf>,

    /// The time zone of playlist times and of dates on screens, such as
    /// `Europe/Berlin`; the host's when unset.
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

    /// Whether the public URL is `https`, when there is one.
    #[must_use]
    pub fn public_url_is_https(&self) -> Option<bool> {
        self.public_url
            .as_deref()
            .map(|url| url.starts_with("https://"))
    }
}

fn parse_public_url(text: &str) -> Result<String, String> {
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
