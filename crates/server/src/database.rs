//! The SQLite database every store shares.

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("database: {0}")]
    Open(#[from] sqlx::Error),
    #[error("database migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

/// Opens the database at `path`, creating and migrating it as needed. Foreign keys
/// are enforced (sqlx turns them on for every connection).
///
/// # Errors
///
/// When the database cannot be opened or migrated.
pub async fn open(path: &Path) -> Result<SqlitePool, OpenError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
