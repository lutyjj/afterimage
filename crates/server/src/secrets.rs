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

    /// `text` with every secret value in it replaced by its name, so that an error
    /// message, or an answer that echoes its request, never carries one. Values
    /// shorter than [`REDACTED_LENGTH`] are left alone: replacing every `1` would
    /// garble answers and hides nothing.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for (name, value) in &self.0 {
            if value.chars().count() >= REDACTED_LENGTH && text.contains(value.as_str()) {
                text = text.replace(value.as_str(), &format!("[secret {name}]"));
            }
        }
        text
    }

    /// Every string in `value`, keys included, redacted.
    pub fn redact_json(&self, value: &mut serde_json::Value) {
        use serde_json::Value;
        match value {
            Value::String(text) => *text = self.redact(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.redact_json(item)),
            Value::Object(members) => {
                let redacted = std::mem::take(members)
                    .into_iter()
                    .map(|(key, mut member)| {
                        self.redact_json(&mut member);
                        (self.redact(&key), member)
                    })
                    .collect();
                *members = redacted;
            }
            _ => {}
        }
    }
}

/// The shortest secret value [`Secrets::redact`] replaces.
pub const REDACTED_LENGTH: usize = 4;

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

    #[test]
    fn redacts_values_in_text_and_json_but_not_short_ones() {
        let secrets = Secrets::from_vars([
            ("AFTERIMAGE_SECRET_TOKEN".into(), "s3cret-value".into()),
            ("AFTERIMAGE_SECRET_PIN".into(), "123".into()),
        ]);
        assert_eq!(
            secrets.redact("GET /?k=s3cret-value failed, pin 123"),
            "GET /?k=[secret TOKEN] failed, pin 123"
        );
        let mut answer = serde_json::json!({ "headers": { "Authorization": "Bearer s3cret-value" }, "s3cret-value": [1, "s3cret-value"] });
        secrets.redact_json(&mut answer);
        assert_eq!(
            answer,
            serde_json::json!({ "headers": { "Authorization": "Bearer [secret TOKEN]" }, "[secret TOKEN]": [1, "[secret TOKEN]"] })
        );
    }
}
