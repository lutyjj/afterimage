//! Settings from the environment.

use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Address the server listens on; devices on the network must reach it.
pub const BIND: &str = "AFTERIMAGE_BIND";
/// Directory that holds the database.
pub const DATA_DIR: &str = "AFTERIMAGE_DATA_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
}

impl Config {
    /// Reads the settings, with defaults for those not set.
    ///
    /// # Errors
    ///
    /// When a set value does not parse.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, String> {
        let bind = match lookup(BIND) {
            None => "0.0.0.0:8080".to_owned(),
            Some(value) => value
                .into_string()
                .map_err(|_| format!("{BIND} is not valid UTF-8"))?,
        };
        Ok(Self {
            bind: bind
                .parse()
                .map_err(|error| format!("{BIND}={bind}: {error}"))?,
            data_dir: lookup(DATA_DIR).map_or_else(|| PathBuf::from("data"), PathBuf::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_every_interface_and_a_local_directory() {
        let config = Config::from_lookup(|_| None).unwrap();
        assert_eq!(config.bind, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("data"));
    }

    #[test]
    fn names_the_setting_that_does_not_parse() {
        let error =
            Config::from_lookup(|name| (name == BIND).then(|| "nowhere".into())).unwrap_err();
        assert!(error.starts_with("AFTERIMAGE_BIND=nowhere"), "{error}");
    }

    #[test]
    fn takes_any_path_and_names_a_bind_address_that_is_not_text() {
        use std::os::unix::ffi::OsStringExt;
        let raw = || Some(OsString::from_vec(vec![b'd', 0xff]));
        let config =
            Config::from_lookup(|name| if name == DATA_DIR { raw() } else { None }).unwrap();
        assert_eq!(
            config.data_dir,
            PathBuf::from(OsString::from_vec(vec![b'd', 0xff]))
        );
        let error =
            Config::from_lookup(|name| if name == BIND { raw() } else { None }).unwrap_err();
        assert_eq!(error, "AFTERIMAGE_BIND is not valid UTF-8");
    }
}
