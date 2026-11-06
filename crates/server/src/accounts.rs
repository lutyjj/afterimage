//! Accounts and the two ways to prove one: a password (argon2id through
//! `password-auth`), or an identity the OIDC provider verified. Both end in an [`Account`]; sessions ([`crate::sessions`])
//! and every handler see only that.

use std::num::NonZero;
use std::sync::{Arc, LazyLock};

use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use crate::clock::Clock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub name: String,
    /// Set for an account that can sign in with a password.
    pub username: Option<String>,
    /// The account administers the server: the first account does.
    pub administrator: bool,
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
        // Checked before the hash as well as under the lock, so that sign-ups after
        // the first cost no hash.
        if self.exist().await? {
            return Ok(None);
        }
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
            "INSERT INTO accounts (name, username, password_hash, created_at, administrator)
             VALUES (?, ?, ?, ?, 1) RETURNING id",
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
            administrator: true,
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
            r#"SELECT id, name, password_hash AS "password_hash!", administrator AS "administrator: bool"
               FROM accounts WHERE username = ? AND password_hash IS NOT NULL"#,
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
            administrator: row.administrator,
        }))
    }

    /// The account the provider's `issuer` and `subject` identify, created on first
    /// sign-in with `name`; the server's administrator when no account exists yet,
    /// as one statement decides.
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
            "INSERT INTO accounts (name, oidc_issuer, oidc_subject, created_at, administrator)
             VALUES (?, ?, ?, ?, NOT EXISTS (SELECT 1 FROM accounts))
             ON CONFLICT (oidc_issuer, oidc_subject) DO NOTHING",
            name,
            issuer,
            subject,
            now
        )
        .execute(&self.pool)
        .await?;
        Ok(sqlx::query_as!(
            Account,
            r#"SELECT id, name, username, administrator AS "administrator: bool"
               FROM accounts WHERE oidc_issuer = ? AND oidc_subject = ?"#,
            issuer,
            subject
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn get(&self, id: i64) -> Result<Option<Account>, Error> {
        Ok(sqlx::query_as!(
            Account,
            r#"SELECT id, name, username, administrator AS "administrator: bool"
               FROM accounts WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?)
    }
}

/// An argon2id hash of a password nobody has, made at the default cost on first
/// use and verified against when a username is unknown.
static UNKNOWN_USER_HASH: LazyLock<String> =
    LazyLock::new(|| password_auth::generate_hash("no account has this password"));

/// Hashes running at once. Each holds argon2id's default 19 MiB, and requests
/// without a session start them, so they wait for a place rather than all run.
static HASHING: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(
        std::thread::available_parallelism().map_or(2, NonZero::get),
    ))
});

/// Runs password work on the blocking pool once one of `places` is free. The
/// work holds its place until it finishes, even when the request that started it
/// is dropped, since blocking work cannot be cancelled.
async fn bounded<T: Send + 'static>(
    places: &Arc<Semaphore>,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let place = Arc::clone(places).acquire_owned().await.ok()?;
    tokio::task::spawn_blocking(move || {
        let done = work();
        drop(place);
        done
    })
    .await
    .ok()
}

/// The argon2id PHC string for this password, at `RustCrypto`'s default cost.
async fn hash(password: String) -> Result<String, Error> {
    bounded(&HASHING, move || password_auth::generate_hash(password))
        .await
        .ok_or(Error::Hash)
}

/// Whether this password produced that hash. A stored hash that does not parse
/// answers no.
async fn verify(password: String, stored: String) -> bool {
    bounded(&HASHING, move || {
        password_auth::verify_password(password, &stored).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Spends what a verify costs, for a username that has no account. The hash is
/// first read on the blocking pool, since that read makes it.
async fn verify_unknown(password: String) {
    bounded(&HASHING, move || {
        password_auth::verify_password(password, &UNKNOWN_USER_HASH).is_ok()
    })
    .await;
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn password_work_runs_at_most_as_many_at_once_as_there_are_places() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let places = Arc::new(Semaphore::new(2));
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let start = |places: &Arc<Semaphore>| {
            let (places, running, most) = (places.clone(), running.clone(), most.clone());
            tokio::spawn(async move {
                bounded(&places, move || {
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    running.fetch_sub(1, Ordering::SeqCst);
                })
                .await
            })
        };
        // Requests dropped while their work runs, as a client that disconnects:
        // aborted once both places hold running work.
        let dropped: Vec<_> = (0..4).map(|_| start(&places)).collect();
        while running.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
        for task in &dropped {
            task.abort();
        }
        let tasks: Vec<_> = (0..12).map(|_| start(&places)).collect();
        for task in tasks {
            assert!(task.await.unwrap().is_some());
        }
        assert_eq!(most.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_unknown_user_hash_matches_no_password_tried() {
        assert!(UNKNOWN_USER_HASH.starts_with("$argon2id$"));
        assert!(!verify(String::new(), UNKNOWN_USER_HASH.clone()).await);
    }
}
