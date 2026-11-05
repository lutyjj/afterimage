//! The deployment's secrets, which data source requests read by name, such as a
//! Home Assistant token. Each comes from an environment variable
//! `AFTERIMAGE_SECRET_<NAME>`; screens name them and never hold them.

use std::collections::BTreeMap;
use std::ffi::OsString;

const PREFIX: &str = "AFTERIMAGE_SECRET_";

#[derive(Debug, Clone, Default)]
pub struct Secrets(BTreeMap<String, String>);

impl Secrets {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_vars(std::env::vars_os())
    }

    /// Reads `AFTERIMAGE_SECRET_<NAME>` variables whose name is letters, digits and
    /// underscores and whose value is text, and skips the rest with a warning.
    pub fn from_vars(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        let mut secrets = BTreeMap::new();
        for (name, value) in vars {
            let Some(name) = name.to_str().and_then(|name| name.strip_prefix(PREFIX)) else {
                continue;
            };
            let valid =
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            if let (true, Ok(value)) = (valid, value.into_string()) {
                secrets.insert(name.to_owned(), value);
            } else {
                tracing::warn!(
                    variable = format!("{PREFIX}{name}"),
                    "secret skipped: its name must be letters, digits and underscores, and its value text"
                );
            }
        }
        Self(secrets)
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_prefixed_variables_by_their_suffix() {
        let secrets = Secrets::from_vars([
            ("AFTERIMAGE_SECRET_HA_TOKEN".into(), "abc".into()),
            ("AFTERIMAGE_SECRET_".into(), "empty name".into()),
            ("AFTERIMAGE_SECRET_BAD-NAME".into(), "x".into()),
            ("AFTERIMAGE_BIND".into(), "0.0.0.0:80".into()),
        ]);
        assert_eq!(secrets.names(), ["HA_TOKEN"]);
        assert_eq!(secrets.values()["HA_TOKEN"], "abc");
    }
}
