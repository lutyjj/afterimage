//! The device lifecycle and its storage.
//!
//! Any device may run setup: it gets a key and a friendly ID, and waits for an
//! account to claim it by that ID. Setup for a MAC the server knows issues a new key
//! and keeps the ID. A device still unclaimed after [`CLAIM_WINDOW_SECONDS`] starts
//! over: its key stops working, and its next setup gives it a new ID.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use afterimage_trmnl::{Mac, Readings};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::clock::Clock;
use crate::secrets::{self, Entropy};

/// How long a device may wait to be claimed.
pub const CLAIM_WINDOW_SECONDS: i64 = 24 * 60 * 60;
/// Devices that may wait to be claimed at once. Setup refuses a new MAC beyond it,
/// which bounds what anyone on the network can make the server store.
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
    /// [`UNCLAIMED_LIMIT`] devices are already waiting to be claimed.
    UnclaimedLimit,
}

/// A device found by its key.
#[derive(Debug, PartialEq, Eq)]
pub struct KnownDevice {
    pub friendly_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("device storage migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("no free friendly ID after {FRIENDLY_ID_ATTEMPTS} attempts")]
    FriendlyIdsExhausted,
}

pub struct Registry {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
}

impl Registry {
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

    /// Registers the device with this MAC, or issues it a new key if it is known.
    ///
    /// # Errors
    ///
    /// When storage fails.
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
        let expired = now - CLAIM_WINDOW_SECONDS;
        let key = secrets::new_key(&*self.entropy);
        let key_hash = secrets::key_hash(&key);

        // IMMEDIATE takes the write lock up front, so the count and the insert below
        // see the same set of devices as every other setup.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query!("DELETE FROM devices WHERE created_at <= ?", expired)
            .execute(&mut *tx)
            .await?;
        let known = sqlx::query_scalar!(
            "UPDATE devices SET key_hash = ?, model = ? WHERE mac = ? RETURNING friendly_id",
            key_hash,
            model,
            mac
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(friendly_id) = known {
            tx.commit().await?;
            return Ok(Setup::Registered { key, friendly_id });
        }

        let unclaimed = sqlx::query_scalar!("SELECT COUNT(*) FROM devices")
            .fetch_one(&mut *tx)
            .await?;
        if unclaimed >= UNCLAIMED_LIMIT {
            tx.commit().await?;
            return Ok(Setup::Refused(Refusal::UnclaimedLimit));
        }
        for _ in 0..FRIENDLY_ID_ATTEMPTS {
            let friendly_id = secrets::new_friendly_id(&*self.entropy);
            let inserted = sqlx::query!(
                "INSERT INTO devices (mac, friendly_id, key_hash, model, created_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (friendly_id) DO NOTHING",
                mac,
                friendly_id,
                key_hash,
                model,
                now
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
    ) -> Result<Option<KnownDevice>, Error> {
        let key_hash = secrets::key_hash(key);
        let now = self.clock.now();
        let expired = now - CLAIM_WINDOW_SECONDS;
        let friendly_id = sqlx::query_scalar!(
            "UPDATE devices SET
                 seen_at = ?,
                 battery_voltage = COALESCE(?, battery_voltage),
                 battery_percent = COALESCE(?, battery_percent),
                 signal_dbm = COALESCE(?, signal_dbm),
                 firmware_version = COALESCE(?, firmware_version)
             WHERE key_hash = ? AND created_at > ?
             RETURNING friendly_id",
            now,
            readings.battery_voltage,
            readings.battery_percent,
            readings.signal_dbm,
            readings.firmware_version,
            key_hash,
            expired
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(friendly_id.map(|friendly_id| KnownDevice { friendly_id }))
    }

    /// The device that holds this key, without recording anything.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn find(&self, key: &str) -> Result<Option<KnownDevice>, Error> {
        let key_hash = secrets::key_hash(key);
        let expired = self.clock.now() - CLAIM_WINDOW_SECONDS;
        let friendly_id = sqlx::query_scalar!(
            "SELECT friendly_id FROM devices WHERE key_hash = ? AND created_at > ?",
            key_hash,
            expired
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(friendly_id.map(|friendly_id| KnownDevice { friendly_id }))
    }
}
