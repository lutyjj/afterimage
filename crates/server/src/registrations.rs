//! Registrations: one per setup, holding the key a device uses and the code it shows.
//!
//! Setup always creates a new registration and never touches an existing one, so no
//! request can take a key away from a device. That matters because the firmware has
//! no way back from a lost key except a wipe of its Wi-Fi settings (see
//! [`afterimage_trmnl::DisplayStatus::Reset`]), and because anyone on the network can
//! present a MAC: a spoofed setup gets a registration of its own, whose code never
//! appears on the real device's screen and so cannot be claimed.
//!
//! A device that runs setup again shows the new code; the registration it left
//! behind goes idle. Unclaimed registrations end when they are not used: one whose
//! key never checks in after [`UNUSED_SECONDS`], one that stops checking in after
//! [`IDLE_SECONDS`]. A powered device checks in at least hourly. Each row carries
//! the time it ends (`expires_at`), which setup and check-in set; every query treats
//! an ended registration as gone, and setup deletes ended rows.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use afterimage_trmnl::{Mac, Readings};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::clock::Clock;
use crate::secrets::{self, Entropy};

/// How long a registration whose key never checks in holds its place.
pub const UNUSED_SECONDS: i64 = 24 * 60 * 60;
/// How long an unclaimed registration lasts without a check-in.
pub const IDLE_SECONDS: i64 = 7 * 24 * 60 * 60;
/// Unclaimed registrations at once. Setup refuses beyond it, which bounds what anyone
/// on the network can make the server store.
pub const UNCLAIMED_LIMIT: i64 = 32;
/// Longest firmware model name setup accepts.
const MODEL_LENGTH: usize = 64;
/// Fresh friendly IDs tried before setup gives up. Each collides with an existing
/// one with odds of at most `UNCLAIMED_LIMIT` in 2^30.
const FRIENDLY_ID_ATTEMPTS: usize = 8;

/// The outcome of setup.
#[derive(Debug, PartialEq, Eq)]
pub enum Setup {
    /// The device stores the key and shows the friendly ID until it is claimed.
    Registered {
        key: String,
        friendly_id: String,
    },
    Refused(Refusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    InvalidMac,
    InvalidModel,
    /// [`UNCLAIMED_LIMIT`] registrations are already waiting to be claimed.
    UnclaimedLimit,
}

/// A registration found by its key.
#[derive(Debug, PartialEq, Eq)]
pub struct Registration {
    pub friendly_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("registration storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("registration storage migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("no free friendly ID after {FRIENDLY_ID_ATTEMPTS} attempts")]
    FriendlyIdsExhausted,
}

pub struct Registrations {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
}

impl Registrations {
    /// Opens the database at `path`, creating and migrating it as needed.
    ///
    /// # Errors
    ///
    /// When the database cannot be opened or migrated.
    pub async fn open(
        path: &Path,
        clock: Arc<dyn Clock>,
        entropy: Arc<dyn Entropy>,
    ) -> Result<Self, Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self {
            pool,
            clock,
            entropy,
        })
    }

    /// Registers a device: a new key and friendly ID, whatever registrations its MAC
    /// already has.
    ///
    /// # Errors
    ///
    /// When storage fails, or no free friendly ID turns up.
    pub async fn setup(&self, mac: &str, model: &str) -> Result<Setup, Error> {
        let Ok(mac) = Mac::from_str(mac) else {
            return Ok(Setup::Refused(Refusal::InvalidMac));
        };
        let model = model.trim();
        if model.is_empty()
            || model.len() > MODEL_LENGTH
            || !model.bytes().all(|b| b.is_ascii_graphic())
        {
            return Ok(Setup::Refused(Refusal::InvalidModel));
        }
        let mac = mac.to_string();
        let now = self.clock.now();
        let expires_at = now + UNUSED_SECONDS;
        let key = secrets::new_key(&*self.entropy);
        let key_hash = secrets::key_hash(&key);

        // IMMEDIATE takes the write lock up front, so the count and the insert below
        // see the same registrations as every other setup.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query!("DELETE FROM registrations WHERE expires_at <= ?", now)
            .execute(&mut *tx)
            .await?;
        let unclaimed = sqlx::query_scalar!("SELECT COUNT(*) FROM registrations")
            .fetch_one(&mut *tx)
            .await?;
        if unclaimed >= UNCLAIMED_LIMIT {
            tx.commit().await?;
            return Ok(Setup::Refused(Refusal::UnclaimedLimit));
        }
        for _ in 0..FRIENDLY_ID_ATTEMPTS {
            let friendly_id = secrets::new_friendly_id(&*self.entropy);
            let inserted = sqlx::query!(
                "INSERT INTO registrations (mac, friendly_id, key_hash, model, created_at, expires_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT (friendly_id) DO NOTHING",
                mac,
                friendly_id,
                key_hash,
                model,
                now,
                expires_at
            )
            .execute(&mut *tx)
            .await?;
            if inserted.rows_affected() == 1 {
                tx.commit().await?;
                return Ok(Setup::Registered { key, friendly_id });
            }
        }
        Err(Error::FriendlyIdsExhausted)
    }

    /// Records a check-in: the time, and each reading the device reported.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn check_in(
        &self,
        key: &str,
        readings: &Readings,
    ) -> Result<Option<Registration>, Error> {
        let key_hash = secrets::key_hash(key);
        let now = self.clock.now();
        let expires_at = now + IDLE_SECONDS;
        let friendly_id = sqlx::query_scalar!(
            "UPDATE registrations SET
                 seen_at = ?,
                 expires_at = ?,
                 battery_voltage = COALESCE(?, battery_voltage),
                 battery_percent = COALESCE(?, battery_percent),
                 signal_dbm = COALESCE(?, signal_dbm),
                 firmware_version = COALESCE(?, firmware_version)
             WHERE key_hash = ? AND expires_at > ?
             RETURNING friendly_id",
            now,
            expires_at,
            readings.battery_voltage,
            readings.battery_percent,
            readings.signal_dbm,
            readings.firmware_version,
            key_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(friendly_id.map(|friendly_id| Registration { friendly_id }))
    }

    /// The registration that holds this key, without recording anything.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn find(&self, key: &str) -> Result<Option<Registration>, Error> {
        let key_hash = secrets::key_hash(key);
        let now = self.clock.now();
        let friendly_id = sqlx::query_scalar!(
            "SELECT friendly_id FROM registrations WHERE key_hash = ? AND expires_at > ?",
            key_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(friendly_id.map(|friendly_id| Registration { friendly_id }))
    }
}
