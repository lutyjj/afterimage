//! An account's screens. A document is stored as the JSON of the model's
//! `ScreenDocument`; the management API converts it, and the renderer reads it.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::clock::Clock;

/// Names a screen's templates read besides its data sources, which no data source
/// may take: the time of the render, the device, the deployment's secrets, and in
/// an `each` the element and the count left out.
pub const TEMPLATE_NAMES: [&str; 5] = ["now", "device", "secrets", "item", "hidden"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub id: i64,
    pub name: String,
    /// JSON in the shape of the model's `ScreenDocument`.
    pub document: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub id: i64,
    pub name: String,
    pub updated_at: i64,
}

pub struct Screens {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
}

impl Screens {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// The account's screens without their documents.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn list(&self, account_id: i64) -> Result<Vec<Summary>, sqlx::Error> {
        sqlx::query_as!(
            Summary,
            "SELECT id, name, updated_at FROM screens WHERE account_id = ? ORDER BY name, id",
            account_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn create(
        &self,
        account_id: i64,
        name: &str,
        document: &str,
    ) -> Result<Screen, sqlx::Error> {
        let now = self.clock.now();
        sqlx::query_as!(
            Screen,
            "INSERT INTO screens (account_id, name, document, updated_at) VALUES (?, ?, ?, ?)
             RETURNING id AS \"id!\", name, document, updated_at",
            account_id,
            name,
            document,
            now
        )
        .fetch_one(&self.pool)
        .await
    }

    /// The account's screen; `None` when it has no such screen.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self, account_id: i64, id: i64) -> Result<Option<Screen>, sqlx::Error> {
        sqlx::query_as!(
            Screen,
            "SELECT id, name, document, updated_at FROM screens WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn update(
        &self,
        account_id: i64,
        id: i64,
        name: &str,
        document: &str,
    ) -> Result<Option<Screen>, sqlx::Error> {
        let now = self.clock.now();
        sqlx::query_as!(
            Screen,
            "UPDATE screens SET name = ?, document = ?, updated_at = ?
             WHERE id = ? AND account_id = ?
             RETURNING id, name, document, updated_at",
            name,
            document,
            now,
            id,
            account_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Deletes the screen, and with it its place in every playlist; a device
    /// that slept on it shows the server's own sleep screen instead, and one
    /// pinned on it goes back to its schedule. `false` when the account has no
    /// such screen.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn delete(&self, account_id: i64, id: i64) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        // A device going through a playlist that loses a screen starts it again
        // at its first.
        sqlx::query!(
            "UPDATE devices SET playlist_position = NULL
             WHERE account_id = ?
               AND position_playlist_id IN (SELECT playlist_id FROM playlist_screens WHERE screen_id = ?)",
            account_id,
            id
        )
        .execute(&mut *tx)
        .await?;
        let deleted = sqlx::query!(
            "DELETE FROM screens WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(deleted.rows_affected() == 1)
    }
}
