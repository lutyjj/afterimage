//! Personal API tokens: what scripts and agents sign in with, as
//! `Authorization: Bearer <secret>`. A token acts as its owner wherever a session
//! does, narrowed to reading when it is read-only, and lasts until it is
//! revoked. The secret is a random token like a session's, and the database
//! holds only its SHA-256, so a copy of the database signs nobody in.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::accounts::Account;
use crate::clock::Clock;
use crate::keys::{self, Entropy};

/// A token as its owner reads it back: never its secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub id: i64,
    pub name: String,
    pub read_only: bool,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// The account a token acts as, and whether it may only read.
#[derive(Debug, Clone)]
pub struct Grant {
    pub account: Account,
    pub read_only: bool,
}

pub struct Tokens {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
}

impl Tokens {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>, entropy: Arc<dyn Entropy>) -> Self {
        Self {
            pool,
            clock,
            entropy,
        }
    }

    /// Creates a token for the account and returns it with its secret, which
    /// nothing returns again.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn create(
        &self,
        account_id: i64,
        name: &str,
        read_only: bool,
    ) -> Result<(Token, String), sqlx::Error> {
        let secret = keys::new_token(&*self.entropy);
        let token_hash = keys::token_hash(&secret);
        let now = self.clock.now();
        let id = sqlx::query_scalar!(
            r#"INSERT INTO access_tokens (account_id, name, token_hash, read_only, created_at)
               VALUES (?, ?, ?, ?, ?) RETURNING id AS "id!""#,
            account_id,
            name,
            token_hash,
            read_only,
            now
        )
        .fetch_one(&self.pool)
        .await?;
        let token = Token {
            id,
            name: name.to_owned(),
            read_only,
            created_at: now,
            last_used_at: None,
        };
        Ok((token, secret))
    }

    /// The account's tokens, newest first.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn list(&self, account_id: i64) -> Result<Vec<Token>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT id, name, read_only AS "read_only: bool", created_at, last_used_at
               FROM access_tokens WHERE account_id = ? ORDER BY created_at DESC, id DESC"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| Token {
                id: row.id,
                name: row.name,
                read_only: row.read_only,
                created_at: row.created_at,
                last_used_at: row.last_used_at,
            })
            .collect())
    }

    /// Revokes the account's token; `false` when it has no such token.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn revoke(&self, account_id: i64, id: i64) -> Result<bool, sqlx::Error> {
        let revoked = sqlx::query!(
            "DELETE FROM access_tokens WHERE id = ? AND account_id = ?",
            id,
            account_id
        )
        .execute(&self.pool)
        .await?;
        Ok(revoked.rows_affected() == 1)
    }

    /// The grant of a token's secret, recording the use.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn grant(&self, secret: &str) -> Result<Option<Grant>, sqlx::Error> {
        let token_hash = keys::token_hash(secret);
        let now = self.clock.now();
        let row = sqlx::query!(
            r#"UPDATE access_tokens SET last_used_at = ? WHERE token_hash = ?
               RETURNING account_id, read_only AS "read_only: bool""#,
            now,
            token_hash
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let account = sqlx::query!(
            "SELECT id, name, username FROM accounts WHERE id = ?",
            row.account_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(Some(Grant {
            account: Account {
                id: account.id,
                name: account.name,
                username: account.username,
            },
            read_only: row.read_only,
        }))
    }
}
