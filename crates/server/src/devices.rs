//! Claimed devices: an account's name, display settings and playlist for the
//! registration a device holds.
//!
//! Claiming moves a registration out of the unclaimed state ([`crate::registrations`])
//! in the same transaction that creates or updates the device. A claimed MAC that
//! matches one of the account's devices is the same device set up again: it takes
//! the new registration and keeps everything else, and its old registration ends.
//! Other unclaimed registrations of the MAC are left to end on their own, since a
//! claim cannot tell the device's leftovers from someone else's setups.
//! Removing a device hands its registration back to the unclaimed state, so the
//! device shows its code again, or marks it for a reset.

use std::sync::Arc;

use afterimage_trmnl::Readings;
use afterimage_trmnl::catalog::Catalog;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::clock::Clock;
use crate::registrations::IDLE_SECONDS;

/// A new device's minutes between check-ins, TRMNL's default refresh rate.
pub const DEFAULT_REFRESH_MINUTES: i64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    /// A model name from TRMNL's catalogue.
    pub model: String,
    /// A palette id the model supports.
    pub palette: String,
}

/// One playlist entry. Days are bits, Monday as 1 through Sunday as 64; times are
/// minutes after midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub screen_id: i64,
    pub days: Option<u8>,
    pub from_minute: Option<u16>,
    pub until_minute: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub mac: String,
    pub firmware_model: String,
    pub display: Display,
    pub refresh_minutes: i64,
    pub playlist: Vec<Entry>,
    pub readings: Readings,
    pub seen_at: Option<i64>,
}

/// What a claim did.
#[derive(Debug)]
pub enum Claim {
    /// The code belongs to no unclaimed registration.
    NoSuchCode,
    Claimed(Device),
    /// The account's device with the same MAC took the registration.
    Replaced(Device),
}

/// Changes to a device's settings; `None` keeps a setting.
#[derive(Debug, Default)]
pub struct Changes {
    pub name: Option<String>,
    pub display: Option<Display>,
    pub refresh_minutes: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("{model} has no palette {palette}")]
    UnsupportedDisplay { model: String, palette: String },
    #[error("screen {0} does not exist")]
    UnknownScreen(i64),
}

pub struct Devices {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
}

impl Devices {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Claims the unclaimed registration showing `code` for the account.
    ///
    /// # Errors
    ///
    /// When storage fails.
    ///
    /// # Panics
    ///
    /// When the device written in the transaction cannot be read back in it.
    pub async fn claim(
        &self,
        account_id: i64,
        code: &str,
        name: Option<&str>,
    ) -> Result<Claim, Error> {
        let now = self.clock.now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(registration) = sqlx::query!(
            "SELECT id, mac, model, width, height FROM registrations
             WHERE friendly_id = ? AND expires_at > ? AND reset = 0",
            code,
            now
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(Claim::NoSuchCode);
        };
        let existing = sqlx::query!(
            "SELECT devices.id, devices.registration_id FROM devices
             JOIN registrations ON registrations.id = devices.registration_id
             WHERE devices.account_id = ? AND registrations.mac = ?",
            account_id,
            registration.mac
        )
        .fetch_optional(&mut *tx)
        .await?;
        let (device_id, replaced) = if let Some(existing) = existing {
            sqlx::query!(
                "UPDATE devices SET registration_id = ?, name = COALESCE(?, name) WHERE id = ?",
                registration.id,
                name,
                existing.id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "DELETE FROM registrations WHERE id = ?",
                existing.registration_id
            )
            .execute(&mut *tx)
            .await?;
            (existing.id, true)
        } else {
            let size = registration
                .width
                .zip(registration.height)
                .and_then(|(width, height)| {
                    Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?))
                });
            let (model, palette) = Catalog::get().default_display(&registration.model, size);
            let name = name.map_or_else(|| format!("Device {code}"), str::to_owned);
            let id = sqlx::query_scalar!(
                "INSERT INTO devices
                     (account_id, registration_id, name, display_model, palette, refresh_minutes, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id AS \"id!\"",
                account_id,
                registration.id,
                name,
                model.name,
                palette.id,
                DEFAULT_REFRESH_MINUTES,
                now
            )
            .fetch_one(&mut *tx)
            .await?;
            (id, false)
        };
        sqlx::query!(
            "UPDATE registrations SET expires_at = NULL WHERE id = ?",
            registration.id
        )
        .execute(&mut *tx)
        .await?;
        let device = load(&mut tx, account_id, device_id)
            .await?
            .expect("the device was just written");
        tx.commit().await?;
        Ok(if replaced {
            Claim::Replaced(device)
        } else {
            Claim::Claimed(device)
        })
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn list(&self, account_id: i64) -> Result<Vec<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        let ids = sqlx::query_scalar!(
            "SELECT id FROM devices WHERE account_id = ? ORDER BY name, id",
            account_id
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut devices = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(device) = load(&mut tx, account_id, id).await? {
                devices.push(device);
            }
        }
        Ok(devices)
    }

    /// The account's device; `None` when it has no such device.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self, account_id: i64, id: i64) -> Result<Option<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        Ok(load(&mut tx, account_id, id).await?)
    }

    /// The device holding a registration, whichever account owns it.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn holding(&self, registration_id: i64) -> Result<Option<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query!(
            "SELECT id, account_id FROM devices WHERE registration_id = ?",
            registration_id
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        Ok(load(&mut tx, row.account_id, row.id).await?)
    }

    /// # Errors
    ///
    /// `UnsupportedDisplay` for a palette the model does not support or a model the
    /// catalogue lacks; otherwise when storage fails.
    pub async fn update(
        &self,
        account_id: i64,
        id: i64,
        changes: Changes,
    ) -> Result<Option<Device>, Error> {
        if let Some(display) = &changes.display
            && !Catalog::get().supports(&display.model, &display.palette)
        {
            return Err(Error::UnsupportedDisplay {
                model: display.model.clone(),
                palette: display.palette.clone(),
            });
        }
        let (model, palette) = changes
            .display
            .map(|display| (display.model, display.palette))
            .unzip();
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query!(
            "UPDATE devices SET
                 name = COALESCE(?, name),
                 display_model = COALESCE(?, display_model),
                 palette = COALESCE(?, palette),
                 refresh_minutes = COALESCE(?, refresh_minutes)
             WHERE id = ? AND account_id = ?",
            changes.name,
            model,
            palette,
            changes.refresh_minutes,
            id,
            account_id
        )
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        let device = load(&mut tx, account_id, id).await?;
        tx.commit().await?;
        Ok(device)
    }

    /// Replaces the device's playlist.
    ///
    /// # Errors
    ///
    /// `UnknownScreen` for a screen the account does not have; otherwise when
    /// storage fails.
    pub async fn set_playlist(
        &self,
        account_id: i64,
        id: i64,
        playlist: &[Entry],
    ) -> Result<Option<Device>, Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let owned = sqlx::query_scalar!(
            "SELECT id FROM devices WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if owned.is_none() {
            return Ok(None);
        }
        sqlx::query!("DELETE FROM playlist_entries WHERE device_id = ?", id)
            .execute(&mut *tx)
            .await?;
        for (position, entry) in (0_i64..).zip(playlist) {
            let screen = sqlx::query_scalar!(
                "SELECT id FROM screens WHERE id = ? AND account_id = ?",
                entry.screen_id,
                account_id
            )
            .fetch_optional(&mut *tx)
            .await?;
            if screen.is_none() {
                return Err(Error::UnknownScreen(entry.screen_id));
            }
            sqlx::query!(
                "INSERT INTO playlist_entries (device_id, position, screen_id, days, from_minute, until_minute)
                 VALUES (?, ?, ?, ?, ?, ?)",
                id,
                position,
                entry.screen_id,
                entry.days,
                entry.from_minute,
                entry.until_minute
            )
            .execute(&mut *tx)
            .await?;
        }
        let device = load(&mut tx, account_id, id).await?;
        tx.commit().await?;
        Ok(device)
    }

    /// Removes the account's device; `false` when it has no such device. Its
    /// registration goes back to unclaimed, or with `reset` waits to erase the
    /// device at its next check-in. Either way it ends after [`IDLE_SECONDS`] without
    /// a check-in.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn delete(&self, account_id: i64, id: i64, reset: bool) -> Result<bool, Error> {
        let expires_at = self.clock.now() + IDLE_SECONDS;
        let mut tx = self.pool.begin().await?;
        let Some(registration_id) = sqlx::query_scalar!(
            "DELETE FROM devices WHERE id = ? AND account_id = ? RETURNING registration_id",
            id,
            account_id
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(false);
        };
        sqlx::query!(
            "UPDATE registrations SET expires_at = ?, reset = ? WHERE id = ?",
            expires_at,
            reset,
            registration_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}

async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: i64,
    id: i64,
) -> Result<Option<Device>, sqlx::Error> {
    let Some(row) = sqlx::query!(
        "SELECT devices.id, devices.name, devices.display_model, devices.palette,
                devices.refresh_minutes, registrations.mac, registrations.model,
                registrations.seen_at, registrations.battery_voltage,
                registrations.battery_percent, registrations.signal_dbm,
                registrations.firmware_version
         FROM devices JOIN registrations ON registrations.id = devices.registration_id
         WHERE devices.id = ? AND devices.account_id = ?",
        id,
        account_id
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let playlist = sqlx::query!(
        "SELECT screen_id, days, from_minute, until_minute FROM playlist_entries
         WHERE device_id = ? ORDER BY position",
        id
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|entry| Entry {
        screen_id: entry.screen_id,
        days: entry.days.and_then(|days| u8::try_from(days).ok()),
        from_minute: entry
            .from_minute
            .and_then(|minute| u16::try_from(minute).ok()),
        until_minute: entry
            .until_minute
            .and_then(|minute| u16::try_from(minute).ok()),
    })
    .collect();
    Ok(Some(Device {
        id: row.id,
        name: row.name,
        mac: row.mac,
        firmware_model: row.model,
        display: Display {
            model: row.display_model,
            palette: row.palette,
        },
        refresh_minutes: row.refresh_minutes,
        playlist,
        readings: Readings {
            battery_voltage: row.battery_voltage,
            battery_percent: row.battery_percent,
            signal_dbm: row.signal_dbm.and_then(|dbm| i32::try_from(dbm).ok()),
            firmware_version: row.firmware_version,
        },
        seen_at: row.seen_at,
    }))
}
