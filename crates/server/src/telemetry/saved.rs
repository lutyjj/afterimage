//! The broker the administrator saved in the web UI, for a deployment that sets
//! none: the one row of `mqtt_settings`.

use sqlx::SqlitePool;

use super::{Settings, broker_url};

/// The broker as saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedBroker {
    pub url: String,
    pub username: Option<String>,
    /// Kept as written, since the server sends it to the broker.
    pub password: Option<String>,
    pub topic: String,
    pub discovery_prefix: String,
}

impl SavedBroker {
    /// The settings to publish with, linking back to the web UI at `public_url`.
    ///
    /// # Errors
    ///
    /// When the saved address is not a broker's, which saving checks.
    pub fn settings(&self, public_url: Option<String>) -> Result<Settings, String> {
        Ok(Settings {
            url: broker_url(&self.url)?,
            username: self.username.clone(),
            password: self.password.clone(),
            topic: self.topic.clone(),
            discovery_prefix: self.discovery_prefix.clone(),
            public_url,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Saved {
    pool: SqlitePool,
}

impl Saved {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self) -> Result<Option<SavedBroker>, sqlx::Error> {
        sqlx::query_as!(
            SavedBroker,
            "SELECT url, username, password, topic, discovery_prefix FROM mqtt_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Saves the broker, or forgets the one saved.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn set(&self, broker: Option<&SavedBroker>, now: i64) -> Result<(), sqlx::Error> {
        match broker {
            None => {
                sqlx::query!("DELETE FROM mqtt_settings")
                    .execute(&self.pool)
                    .await?;
            }
            Some(broker) => {
                sqlx::query!(
                    "INSERT INTO mqtt_settings
                         (id, url, username, password, topic, discovery_prefix, updated_at)
                     VALUES (1, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT (id) DO UPDATE SET
                         url = excluded.url,
                         username = excluded.username,
                         password = excluded.password,
                         topic = excluded.topic,
                         discovery_prefix = excluded.discovery_prefix,
                         updated_at = excluded.updated_at",
                    broker.url,
                    broker.username,
                    broker.password,
                    broker.topic,
                    broker.discovery_prefix,
                    now
                )
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }
}
