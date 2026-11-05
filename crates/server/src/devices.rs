//! The device lifecycle and its storage.
//!
//! Any device may run setup: it gets a key and a friendly ID, and waits for an
//! account to claim it by that ID. The server never invalidates a key a device is
//! using, because the firmware has no way back from a lost key except a wipe of its
//! Wi-Fi settings (see [`afterimage_trmnl::DisplayStatus::Reset`]):
//!
//! - Setup for a known MAC issues a new key only once the current one has been
//!   quiet for [`QUIET_SECONDS`]. Anyone on the network can present a MAC, so the
//!   new key comes with a new friendly ID, which a spoofer cannot show on the real
//!   device's screen.
//! - A registration whose key was never used is dropped [`UNUSED_KEY_SECONDS`]
//!   after the key was issued, which frees its place under [`UNCLAIMED_LIMIT`].

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use afterimage_trmnl::{Mac, Readings};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::clock::Clock;
use crate::secrets::{self, Entropy};

/// How long a key that has never checked in keeps its registration.
pub const UNUSED_KEY_SECONDS: i64 = 24 * 60 * 60;
/// How long a key must go without a check-in before setup replaces it. Unclaimed
/// devices check in every minute or so, so a running device keeps its key.
pub const QUIET_SECONDS: i64 = 30 * 60;
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
    /// The MAC's current key checked in within [`QUIET_SECONDS`].
    KeyInUse,
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

    /// Registers the device with this MAC, or issues a known one a new key and
    /// friendly ID once its current key is quiet.
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
        let unused_before = now - UNUSED_KEY_SECONDS;
        let quiet_before = now - QUIET_SECONDS;
        let key = secrets::new_key(&*self.entropy);
        let key_hash = secrets::key_hash(&key);

        // IMMEDIATE takes the write lock up front, so the lookups and writes below
        // see the same devices as every other setup.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query!(
            "DELETE FROM devices WHERE seen_at IS NULL AND key_issued_at <= ?",
            unused_before
        )
        .execute(&mut *tx)
        .await?;
        let known = sqlx::query!(
            r#"SELECT id, COALESCE(seen_at, key_issued_at) AS "active_at!: i64"
               FROM devices WHERE mac = ?"#,
            mac
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(known) = known {
            if known.active_at > quiet_before {
                tx.commit().await?;
                return Ok(Setup::Refused(Refusal::KeyInUse));
            }
            for friendly_id in self.friendly_ids() {
                let replaced = sqlx::query!(
                    "UPDATE OR IGNORE devices
                     SET key_hash = ?, friendly_id = ?, model = ?, key_issued_at = ?, seen_at = NULL
                     WHERE id = ?",
                    key_hash,
                    friendly_id,
                    model,
                    now,
                    known.id
                )
                .execute(&mut *tx)
                .await?;
                if replaced.rows_affected() == 1 {
                    tx.commit().await?;
                    return Ok(Setup::Registered { key, friendly_id });
                }
            }
            return Err(Error::FriendlyIdsExhausted);
        }

        let unclaimed = sqlx::query_scalar!("SELECT COUNT(*) FROM devices")
            .fetch_one(&mut *tx)
            .await?;
        if unclaimed >= UNCLAIMED_LIMIT {
            tx.commit().await?;
            return Ok(Setup::Refused(Refusal::UnclaimedLimit));
        }
        for friendly_id in self.friendly_ids() {
            let inserted = sqlx::query!(
                "INSERT INTO devices (mac, friendly_id, key_hash, model, created_at, key_issued_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT (friendly_id) DO NOTHING",
                mac,
                friendly_id,
                key_hash,
                model,
                now,
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

    /// Candidate friendly IDs; the caller keeps the first no other device holds.
    fn friendly_ids(&self) -> impl Iterator<Item = String> + '_ {
        (0..FRIENDLY_ID_ATTEMPTS).map(|_| secrets::new_friendly_id(&*self.entropy))
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
        let friendly_id = sqlx::query_scalar!(
            "UPDATE devices SET
                 seen_at = ?,
                 battery_voltage = COALESCE(?, battery_voltage),
                 battery_percent = COALESCE(?, battery_percent),
                 signal_dbm = COALESCE(?, signal_dbm),
                 firmware_version = COALESCE(?, firmware_version)
             WHERE key_hash = ?
             RETURNING friendly_id",
            now,
            readings.battery_voltage,
            readings.battery_percent,
            readings.signal_dbm,
            readings.firmware_version,
            key_hash
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
        let friendly_id = sqlx::query_scalar!(
            "SELECT friendly_id FROM devices WHERE key_hash = ?",
            key_hash
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(friendly_id.map(|friendly_id| KnownDevice { friendly_id }))
    }
}
