//! Install requests: at most one per device, kept until another replaces it.
//!
//! A request waits for the device's next check-in, whose answer carries it once;
//! the check-in after that decides it by the version the device reports. Both
//! firmware lines act on such an answer: 1.8 installs the image, restarts and
//! checks in with the new version, or records the failure and tries again only a
//! day later (`otaAttemptDue` in `lib/trmnl/src/ota_schedule.cpp`); 1.6 installs
//! it and starts it at its next wake (`checkAndPerformFirmwareUpdate` in
//! `src/bl.cpp`). 1.6 has no such pause, so a request sent at every check-in
//! would make a failing device download the image at every wake.

use std::sync::Arc;

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::clock::Clock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub board: String,
    pub version: String,
    /// The image's name in the firmware cache.
    pub image: String,
    pub requested_at: i64,
    pub sent_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// Whether the check-in after the request reported `version`.
    pub installed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Requested,
    Sent,
    Installed,
    NotInstalled,
}

impl Update {
    #[must_use]
    pub fn state(&self) -> State {
        match (self.sent_at, self.installed) {
            (None, _) => State::Requested,
            (Some(_), None) => State::Sent,
            (Some(_), Some(true)) => State::Installed,
            (Some(_), Some(false)) => State::NotInstalled,
        }
    }
}

/// What a check-in's answer says about firmware.
#[derive(Debug, PartialEq, Eq)]
pub enum Delivery {
    Nothing,
    /// The answer carries the request for this cached image.
    Send(String),
    /// The request's image left the cache; it has to be cut again first.
    Missing {
        board: String,
        version: String,
    },
}

pub struct Updates {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
}

impl Updates {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Records a request, replacing the device's last one.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn request(
        &self,
        device_id: i64,
        board: &str,
        version: &str,
        image: &str,
    ) -> Result<(), sqlx::Error> {
        let now = self.clock.now();
        sqlx::query!(
            "INSERT OR REPLACE INTO firmware_updates (device_id, board, version, image, requested_at)
             VALUES (?, ?, ?, ?, ?)",
            device_id,
            board,
            version,
            image,
            now
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Forgets the device's request, whatever its state.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn cancel(&self, device_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM firmware_updates WHERE device_id = ?",
            device_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Moves the device's request on at a check-in where it reports `version`
    /// (its latest known one): one that was sent ends, installed or not; one
    /// that waits is sent when `cached` holds its image.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn check_in(
        &self,
        device_id: i64,
        version: Option<&str>,
        cached: impl Fn(&str) -> bool,
    ) -> Result<Delivery, sqlx::Error> {
        let now = self.clock.now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(update) = load(&mut tx, device_id).await? else {
            return Ok(Delivery::Nothing);
        };
        let delivery = match update.state() {
            State::Installed | State::NotInstalled => Delivery::Nothing,
            State::Sent => {
                let installed = version == Some(update.version.as_str());
                sqlx::query!(
                    "UPDATE firmware_updates SET finished_at = ?, installed = ? WHERE device_id = ?",
                    now,
                    installed,
                    device_id
                )
                .execute(&mut *tx)
                .await?;
                Delivery::Nothing
            }
            State::Requested if cached(&update.image) => {
                sqlx::query!(
                    "UPDATE firmware_updates SET sent_at = ? WHERE device_id = ?",
                    now,
                    device_id
                )
                .execute(&mut *tx)
                .await?;
                Delivery::Send(update.image)
            }
            State::Requested => Delivery::Missing {
                board: update.board,
                version: update.version,
            },
        };
        tx.commit().await?;
        Ok(delivery)
    }

    /// Points a waiting request for `version` at a newly cut image.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn replace_image(
        &self,
        device_id: i64,
        version: &str,
        image: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE firmware_updates SET image = ?
             WHERE device_id = ? AND version = ? AND sent_at IS NULL",
            image,
            device_id,
            version
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The images of requests a device may still download: those waiting and
    /// those sent and not yet decided.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn images_in_use(&self) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar!("SELECT image FROM firmware_updates WHERE finished_at IS NULL")
            .fetch_all(&self.pool)
            .await
    }
}

/// The device's request, read in the caller's transaction.
///
/// # Errors
///
/// When storage fails.
pub async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    device_id: i64,
) -> Result<Option<Update>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT board, version, image, requested_at, sent_at, finished_at,
                  installed AS "installed: bool"
           FROM firmware_updates WHERE device_id = ?"#,
        device_id
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|row| Update {
        board: row.board,
        version: row.version,
        image: row.image,
        requested_at: row.requested_at,
        sent_at: row.sent_at,
        finished_at: row.finished_at,
        installed: row.installed,
    }))
}

/// Withdraws a request the device has not received, as when its board changes.
///
/// # Errors
///
/// When storage fails.
pub async fn withdraw_unsent(
    tx: &mut Transaction<'_, Sqlite>,
    device_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM firmware_updates WHERE device_id = ? AND sent_at IS NULL",
        device_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
