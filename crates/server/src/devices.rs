//! Claimed devices: an account's name, display settings, playlist and sleep for
//! the registration a device holds.
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

use afterimage_render_typst::Renderer;
use afterimage_trmnl::Readings;
use afterimage_trmnl::catalog::Catalog;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::clock::Clock;
use crate::firmware;
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

/// When a playlist entry or a sleep window is current. Days are bits, Monday as
/// 1 through Sunday as 64; times are minutes after midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hours {
    pub days: Option<u8>,
    pub from_minute: Option<u16>,
    pub until_minute: Option<u16>,
}

/// One playlist entry: a screen and when it is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub screen_id: i64,
    pub hours: Hours,
}

/// When a device sleeps, and the screen it shows meanwhile; `None` for the
/// server's own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sleep {
    pub windows: Vec<Hours>,
    pub screen_id: Option<i64>,
}

/// The image a device shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Showing {
    /// The file name in [`crate::images::Images`].
    pub image: String,
    /// The screen it was drawn from; `None` for one of the server's own.
    pub screen_id: Option<i64>,
    pub rendered_at: i64,
    /// Whether it is the device's sleep screen.
    pub asleep: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub mac: String,
    pub firmware_model: String,
    /// The panel's size in pixels as the firmware reported it at check-in.
    pub panel: Option<(u32, u32)>,
    pub display: Display,
    pub refresh_minutes: i64,
    pub playlist: Vec<Entry>,
    pub sleep: Sleep,
    pub readings: Readings,
    pub seen_at: Option<i64>,
    /// When the server asked the device to check in next.
    pub wakes_at: Option<i64>,
    /// The playlist position shown last.
    pub playlist_position: Option<usize>,
    pub showing: Option<Showing>,
    /// The firmware board a user chose, a name from TRMNL's firmware list.
    pub firmware_board: Option<String>,
    /// The latest firmware install request.
    pub firmware_update: Option<firmware::Update>,
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
    /// Choosing another board withdraws an install request the device has not
    /// received, since it names the old board's image.
    pub firmware_board: Option<String>,
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
            let (model, palette) = Catalog::get().default_display(
                &registration.model,
                panel(registration.width, registration.height),
                Renderer::supports,
            );
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

    /// The device whose live registration holds `key`.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn with_key(&self, key: &str) -> Result<Option<Device>, Error> {
        let key_hash = crate::keys::token_hash(key);
        let Some(registration) = sqlx::query_scalar!(
            "SELECT id FROM registrations WHERE key_hash = ? AND expires_at IS NULL",
            key_hash
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        self.holding(registration).await
    }

    /// Records the image a device now shows, the playlist position it came from,
    /// and when the device was asked to check in next.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn show(
        &self,
        id: i64,
        position: Option<usize>,
        showing: &Showing,
        wakes_at: i64,
    ) -> Result<(), Error> {
        let position = position.and_then(|position| i64::try_from(position).ok());
        sqlx::query!(
            "UPDATE devices SET playlist_position = ?, image_name = ?, image_screen_id = ?, rendered_at = ?,
                                image_asleep = ?, wakes_at = ?
             WHERE id = ?",
            position,
            showing.image,
            showing.screen_id,
            showing.rendered_at,
            showing.asleep,
            wakes_at,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The device, whichever account owns it.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn any(&self, id: i64) -> Result<Option<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        let Some(account_id) =
            sqlx::query_scalar!("SELECT account_id FROM devices WHERE id = ?", id)
                .fetch_optional(&mut *tx)
                .await?
        else {
            return Ok(None);
        };
        Ok(load(&mut tx, account_id, id).await?)
    }

    /// Every device of every account.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn all(&self) -> Result<Vec<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query!("SELECT id, account_id FROM devices ORDER BY id")
            .fetch_all(&mut *tx)
            .await?;
        let mut devices = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(device) = load(&mut tx, row.account_id, row.id).await? {
                devices.push(device);
            }
        }
        Ok(devices)
    }

    /// The device whose registration has this MAC, as `Mac` writes it; when
    /// accounts hold devices of one MAC, the one that checked in last, which is
    /// the device itself while the others hold registrations it left.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn with_mac(&self, mac: &str) -> Result<Option<Device>, Error> {
        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query!(
            "SELECT devices.id, devices.account_id FROM devices
             JOIN registrations ON registrations.id = devices.registration_id
             WHERE registrations.mac = ?
             ORDER BY registrations.seen_at IS NULL, registrations.seen_at DESC, devices.id DESC
             LIMIT 1",
            mac
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        Ok(load(&mut tx, row.account_id, row.id).await?)
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
    /// `UnsupportedDisplay` for a display [`crate::drawing::drawable`] refuses;
    /// otherwise when storage fails.
    pub async fn update(
        &self,
        account_id: i64,
        id: i64,
        changes: Changes,
    ) -> Result<Option<Device>, Error> {
        if let Some(display) = &changes.display
            && crate::drawing::drawable(&display.model, &display.palette).is_none()
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
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(board) = sqlx::query_scalar!(
            "SELECT firmware_board FROM devices WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        if changes.firmware_board.is_some() && changes.firmware_board != board {
            firmware::updates::withdraw_unsent(&mut tx, id).await?;
        }
        sqlx::query!(
            "UPDATE devices SET
                 name = COALESCE(?, name),
                 display_model = COALESCE(?, display_model),
                 palette = COALESCE(?, palette),
                 refresh_minutes = COALESCE(?, refresh_minutes),
                 firmware_board = COALESCE(?, firmware_board)
             WHERE id = ?",
            changes.name,
            model,
            palette,
            changes.refresh_minutes,
            changes.firmware_board,
            id
        )
        .execute(&mut *tx)
        .await?;
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
        // A new playlist starts at its first entry.
        sqlx::query!(
            "UPDATE devices SET playlist_position = NULL WHERE id = ?",
            id
        )
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
                entry.hours.days,
                entry.hours.from_minute,
                entry.hours.until_minute
            )
            .execute(&mut *tx)
            .await?;
        }
        let device = load(&mut tx, account_id, id).await?;
        tx.commit().await?;
        Ok(device)
    }

    /// Replaces when the device sleeps and its sleep screen.
    ///
    /// # Errors
    ///
    /// `UnknownScreen` for a sleep screen the account does not have; otherwise
    /// when storage fails.
    pub async fn set_sleep(
        &self,
        account_id: i64,
        id: i64,
        sleep: &Sleep,
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
        if let Some(screen_id) = sleep.screen_id {
            let screen = sqlx::query_scalar!(
                "SELECT id FROM screens WHERE id = ? AND account_id = ?",
                screen_id,
                account_id
            )
            .fetch_optional(&mut *tx)
            .await?;
            if screen.is_none() {
                return Err(Error::UnknownScreen(screen_id));
            }
        }
        sqlx::query!(
            "UPDATE devices SET sleep_screen_id = ? WHERE id = ?",
            sleep.screen_id,
            id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM sleep_windows WHERE device_id = ?", id)
            .execute(&mut *tx)
            .await?;
        for (position, window) in (0_i64..).zip(&sleep.windows) {
            sqlx::query!(
                "INSERT INTO sleep_windows (device_id, position, days, from_minute, until_minute)
                 VALUES (?, ?, ?, ?, ?)",
                id,
                position,
                window.days,
                window.from_minute,
                window.until_minute
            )
            .execute(&mut *tx)
            .await?;
        }
        let device = load(&mut tx, account_id, id).await?;
        tx.commit().await?;
        Ok(device)
    }

    /// Removes the account's device and returns its MAC; `None` when it has no
    /// such device. Its registration goes back to unclaimed, or with `reset` waits
    /// to erase the device at its next check-in. Either way it ends after
    /// [`IDLE_SECONDS`] without a check-in.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn delete(
        &self,
        account_id: i64,
        id: i64,
        reset: bool,
    ) -> Result<Option<String>, Error> {
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
            return Ok(None);
        };
        let mac = sqlx::query_scalar!(
            "UPDATE registrations SET expires_at = ?, reset = ? WHERE id = ? RETURNING mac",
            expires_at,
            reset,
            registration_id
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(mac))
    }
}

/// Days and times as the store keeps them.
fn hours(days: Option<i64>, from_minute: Option<i64>, until_minute: Option<i64>) -> Hours {
    Hours {
        days: days.and_then(|days| u8::try_from(days).ok()),
        from_minute: from_minute.and_then(|minute| u16::try_from(minute).ok()),
        until_minute: until_minute.and_then(|minute| u16::try_from(minute).ok()),
    }
}

/// A panel size as registrations store it.
fn panel(width: Option<i64>, height: Option<i64>) -> Option<(u32, u32)> {
    Some((u32::try_from(width?).ok()?, u32::try_from(height?).ok()?))
}

async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: i64,
    id: i64,
) -> Result<Option<Device>, sqlx::Error> {
    let Some(row) = sqlx::query!(
        "SELECT devices.id, devices.account_id, devices.name, devices.display_model, devices.palette,
                devices.refresh_minutes, devices.playlist_position, devices.image_name,
                devices.image_screen_id, devices.rendered_at, devices.image_asleep,
                devices.firmware_board, devices.sleep_screen_id, devices.wakes_at,
                registrations.mac, registrations.model, registrations.width, registrations.height,
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
        hours: hours(entry.days, entry.from_minute, entry.until_minute),
    })
    .collect();
    let windows = sqlx::query!(
        "SELECT days, from_minute, until_minute FROM sleep_windows
         WHERE device_id = ? ORDER BY position",
        id
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|window| hours(window.days, window.from_minute, window.until_minute))
    .collect();
    let showing = row
        .image_name
        .zip(row.rendered_at)
        .map(|(image, rendered_at)| Showing {
            image,
            screen_id: row.image_screen_id,
            rendered_at,
            asleep: row.image_asleep != 0,
        });
    let firmware_update = firmware::updates::load(tx, id).await?;
    Ok(Some(Device {
        id: row.id,
        account_id: row.account_id,
        name: row.name,
        mac: row.mac,
        firmware_model: row.model,
        panel: panel(row.width, row.height),
        display: Display {
            model: row.display_model,
            palette: row.palette,
        },
        refresh_minutes: row.refresh_minutes,
        playlist,
        sleep: Sleep {
            windows,
            screen_id: row.sleep_screen_id,
        },
        readings: Readings {
            battery_voltage: row.battery_voltage,
            battery_percent: row.battery_percent,
            signal_dbm: row.signal_dbm.and_then(|dbm| i32::try_from(dbm).ok()),
            firmware_version: row.firmware_version,
        },
        seen_at: row.seen_at,
        wakes_at: row.wakes_at,
        playlist_position: row
            .playlist_position
            .and_then(|position| usize::try_from(position).ok()),
        showing,
        firmware_board: row.firmware_board,
        firmware_update,
    }))
}
