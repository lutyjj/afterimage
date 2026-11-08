//! An account's playlists: screens a device shows in turn. Any number of devices
//! show one playlist, through their schedules ([`crate::schedule`]) or pinned,
//! and each goes through it on its own.

use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    /// Screen ids in the order shown.
    pub screens: Vec<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("playlist storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("screen {0} does not exist")]
    UnknownScreen(i64),
}

pub struct Playlists {
    pool: SqlitePool,
}

impl Playlists {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// The account's playlists, by name.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn list(&self, account_id: i64) -> Result<Vec<Playlist>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query!(
            "SELECT id, name FROM playlists WHERE account_id = ? ORDER BY name, id",
            account_id
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut playlists = Vec::with_capacity(rows.len());
        for row in rows {
            playlists.push(Playlist {
                id: row.id,
                name: row.name,
                screens: screens(&mut tx, row.id).await?,
            });
        }
        Ok(playlists)
    }

    /// The account's playlist; `None` when it has no such playlist.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self, account_id: i64, id: i64) -> Result<Option<Playlist>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        load(&mut tx, account_id, id).await
    }

    /// # Errors
    ///
    /// `UnknownScreen` for a screen the account does not have; otherwise when
    /// storage fails.
    ///
    /// # Panics
    ///
    /// When the playlist written in the transaction cannot be read back in it.
    pub async fn create(
        &self,
        account_id: i64,
        name: &str,
        screens: &[i64],
    ) -> Result<Playlist, Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO playlists (account_id, name) VALUES (?, ?) RETURNING id AS \"id!\"",
            account_id,
            name
        )
        .fetch_one(&mut *tx)
        .await?;
        write_screens(&mut tx, account_id, id, screens).await?;
        let playlist = load(&mut tx, account_id, id)
            .await?
            .expect("the playlist was just written");
        tx.commit().await?;
        Ok(playlist)
    }

    /// Replaces the playlist's name and screens. Every device going through it
    /// starts again at its first screen.
    ///
    /// # Errors
    ///
    /// `UnknownScreen` for a screen the account does not have; otherwise when
    /// storage fails.
    pub async fn update(
        &self,
        account_id: i64,
        id: i64,
        name: &str,
        screens: &[i64],
    ) -> Result<Option<Playlist>, Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let renamed = sqlx::query!(
            "UPDATE playlists SET name = ? WHERE id = ? AND account_id = ?",
            name,
            id,
            account_id
        )
        .execute(&mut *tx)
        .await?;
        if renamed.rows_affected() == 0 {
            return Ok(None);
        }
        sqlx::query!("DELETE FROM playlist_screens WHERE playlist_id = ?", id)
            .execute(&mut *tx)
            .await?;
        write_screens(&mut tx, account_id, id, screens).await?;
        sqlx::query!(
            "UPDATE devices SET playlist_position = NULL WHERE position_playlist_id = ?",
            id
        )
        .execute(&mut *tx)
        .await?;
        let playlist = load(&mut tx, account_id, id).await?;
        tx.commit().await?;
        Ok(playlist)
    }

    /// Deletes the playlist, and with it every schedule slot that shows it; a
    /// device that usually showed it, or was pinned on it, no longer does.
    /// `false` when the account has no such playlist.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn delete(&self, account_id: i64, id: i64) -> Result<bool, sqlx::Error> {
        let deleted = sqlx::query!(
            "DELETE FROM playlists WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .execute(&self.pool)
        .await?;
        Ok(deleted.rows_affected() == 1)
    }
}

async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: i64,
    id: i64,
) -> Result<Option<Playlist>, sqlx::Error> {
    let Some(name) = sqlx::query_scalar!(
        "SELECT name FROM playlists WHERE id = ? AND account_id = ?",
        id,
        account_id
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(Playlist {
        id,
        name,
        screens: screens(tx, id).await?,
    }))
}

async fn screens(tx: &mut Transaction<'_, Sqlite>, id: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT screen_id FROM playlist_screens WHERE playlist_id = ? ORDER BY position",
        id
    )
    .fetch_all(&mut **tx)
    .await
}

async fn write_screens(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: i64,
    id: i64,
    screens: &[i64],
) -> Result<(), Error> {
    for (position, &screen_id) in (0_i64..).zip(screens) {
        let owned = sqlx::query_scalar!(
            "SELECT id FROM screens WHERE id = ? AND account_id = ?",
            screen_id,
            account_id
        )
        .fetch_optional(&mut **tx)
        .await?;
        if owned.is_none() {
            return Err(Error::UnknownScreen(screen_id));
        }
        sqlx::query!(
            "INSERT INTO playlist_screens (playlist_id, position, screen_id) VALUES (?, ?, ?)",
            id,
            position,
            screen_id
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
