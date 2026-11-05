//! Accounts and the two ways to prove one: a password (argon2id through
//! `password-auth`), or an identity the OIDC provider verified. Both end in an [`Account`]; sessions ([`crate::sessions`])
//! and every handler see only that.

use std::sync::{Arc, LazyLock};

use sqlx::SqlitePool;

use crate::clock::Clock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub name: String,
    /// Set for an account that can sign in with a password.
    pub username: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("account storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("a password could not be hashed")]
    Hash,
}

pub struct Accounts {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
}

impl Accounts {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Whether any account exists.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn exist(&self) -> Result<bool, Error> {
        Ok(
            sqlx::query_scalar!("SELECT EXISTS (SELECT 1 FROM accounts) AS \"exists!: bool\"")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Creates the first account; `None` when an account already exists.
    ///
    /// # Errors
    ///
    /// When storage or hashing fails.
    pub async fn create_first(
        &self,
        name: &str,
        username: &str,
        password: String,
    ) -> Result<Option<Account>, Error> {
        let password_hash = hash(password).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists =
            sqlx::query_scalar!("SELECT EXISTS (SELECT 1 FROM accounts) AS \"exists!: bool\"")
                .fetch_one(&mut *tx)
                .await?;
        if exists {
            return Ok(None);
        }
        let id = sqlx::query_scalar!(
            "INSERT INTO accounts (name, username, password_hash, created_at)
             VALUES (?, ?, ?, ?) RETURNING id",
            name,
            username,
            password_hash,
            now
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(Account {
            id,
            name: name.to_owned(),
            username: Some(username.to_owned()),
        }))
    }

    /// The account this username and password prove.
    ///
    /// An unknown username costs the same hash as a known one, so the answer's
    /// timing does not tell which usernames exist.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn with_password(
        &self,
        username: &str,
        password: String,
    ) -> Result<Option<Account>, Error> {
        let row = sqlx::query!(
            r#"SELECT id, name, password_hash AS "password_hash!" FROM accounts
               WHERE username = ? AND password_hash IS NOT NULL"#,
            username
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            verify_unknown(password).await;
            return Ok(None);
        };
        Ok(verify(password, row.password_hash).await.then(|| Account {
            id: row.id,
            name: row.name,
            username: Some(username.to_owned()),
        }))
    }

    /// The account the provider's `issuer` and `subject` identify, created on first
    /// sign-in with `name`.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn with_oidc(
        &self,
        issuer: &str,
        subject: &str,
        name: &str,
    ) -> Result<Account, Error> {
        let now = self.clock.now();
        sqlx::query!(
            "INSERT INTO accounts (name, oidc_issuer, oidc_subject, created_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (oidc_issuer, oidc_subject) DO NOTHING",
            name,
            issuer,
            subject,
            now
        )
        .execute(&self.pool)
        .await?;
        let row = sqlx::query!(
            "SELECT id, name, username FROM accounts WHERE oidc_issuer = ? AND oidc_subject = ?",
            issuer,
            subject
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(Account {
            id: row.id,
            name: row.name,
            username: row.username,
        })
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self, id: i64) -> Result<Option<Account>, Error> {
        let row = sqlx::query!("SELECT id, name, username FROM accounts WHERE id = ?", id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| Account {
            id: row.id,
            name: row.name,
            username: row.username,
        }))
    }
}

/// An argon2id hash of a password nobody has, made at the default cost on first
/// use and verified against when a username is unknown.
static UNKNOWN_USER_HASH: LazyLock<String> =
    LazyLock::new(|| password_auth::generate_hash("no account has this password"));

/// The argon2id PHC string for this password, at `RustCrypto`'s default cost. Hashing
/// is deliberately slow, so it runs on the blocking pool.
async fn hash(password: String) -> Result<String, Error> {
    tokio::task::spawn_blocking(move || password_auth::generate_hash(password))
        .await
        .map_err(|_| Error::Hash)
}

/// Whether this password produced that hash. A stored hash that does not parse
/// answers no.
async fn verify(password: String, stored: String) -> bool {
    tokio::task::spawn_blocking(move || password_auth::verify_password(password, &stored).is_ok())
        .await
        .unwrap_or(false)
}

/// Spends what a verify costs, for a username that has no account.
async fn verify_unknown(password: String) {
    verify(password, UNKNOWN_USER_HASH.clone()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_password_verifies_against_its_own_hash_and_no_other() {
        let stored = hash("correct horse battery".to_owned()).await.unwrap();
        assert!(stored.starts_with("$argon2id$"), "{stored}");
        assert!(verify("correct horse battery".to_owned(), stored.clone()).await);
        assert!(!verify("correct horse batter".to_owned(), stored).await);
        let again = hash("correct horse battery".to_owned()).await.unwrap();
        assert_ne!(
            hash("correct horse battery".to_owned()).await.unwrap(),
            again
        );
    }

    #[tokio::test]
    async fn the_unknown_user_hash_matches_no_password_tried() {
        assert!(UNKNOWN_USER_HASH.starts_with("$argon2id$"));
        assert!(!verify(String::new(), UNKNOWN_USER_HASH.clone()).await);
    }
}
