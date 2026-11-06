//! Browser sessions: an opaque random token in an httpOnly cookie, whose SHA-256
//! alone the database holds, so a copy of the database signs nobody in.
//!
//! `tower-sessions` is the common session layer for tower services, but its SQLite
//! store is built against sqlx 0.8, whose `libsqlite3-sys` cannot link beside the
//! sqlx 0.9 this crate uses. A session here is one table and one cookie, built with
//! the `cookie` crate.

use std::sync::Arc;

use cookie::{Cookie, SameSite};
use sqlx::SqlitePool;

use crate::accounts::Account;
use crate::clock::Clock;
use crate::keys::{self, Entropy};

/// How long a session lasts.
pub const SESSION_SECONDS: i64 = 30 * 24 * 60 * 60;

pub struct Sessions {
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
}

impl Sessions {
    #[must_use]
    pub fn new(pool: SqlitePool, clock: Arc<dyn Clock>, entropy: Arc<dyn Entropy>) -> Self {
        Self {
            pool,
            clock,
            entropy,
        }
    }

    /// Starts a session for the account and returns its token.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn start(&self, account_id: i64) -> Result<String, sqlx::Error> {
        let token = keys::new_token(&*self.entropy);
        let token_hash = keys::token_hash(&token);
        let now = self.clock.now();
        let expires_at = now + SESSION_SECONDS;
        sqlx::query!("DELETE FROM sessions WHERE expires_at <= ?", now)
            .execute(&self.pool)
            .await?;
        sqlx::query!(
            "INSERT INTO sessions (token_hash, account_id, expires_at) VALUES (?, ?, ?)",
            token_hash,
            account_id,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(token)
    }

    /// The account of the live session this token starts.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn account(&self, token: &str) -> Result<Option<Account>, sqlx::Error> {
        let token_hash = keys::token_hash(token);
        let now = self.clock.now();
        sqlx::query_as!(
            Account,
            r#"SELECT accounts.id, accounts.name, accounts.username,
                 accounts.administrator AS "administrator: bool"
             FROM sessions JOIN accounts ON accounts.id = sessions.account_id
             WHERE sessions.token_hash = ? AND sessions.expires_at > ?"#,
            token_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// # Errors
    ///
    /// When storage fails.
    pub async fn end(&self, token: &str) -> Result<(), sqlx::Error> {
        let token_hash = keys::token_hash(token);
        sqlx::query!("DELETE FROM sessions WHERE token_hash = ?", token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Whether a browser reached the server over TLS, which decides whether cookies are
/// `Secure` and carry the host-only `__Host-` prefix. The configured public URL
/// decides when there is one; otherwise the reverse proxy's `X-Forwarded-Proto`.
#[derive(Debug, Clone, Copy)]
pub struct CookiePolicy {
    configured: Option<bool>,
}

impl CookiePolicy {
    #[must_use]
    pub fn new(public_url_is_https: Option<bool>) -> Self {
        Self {
            configured: public_url_is_https,
        }
    }

    #[must_use]
    pub fn secure(&self, headers: &http::HeaderMap) -> bool {
        self.configured
            .unwrap_or_else(|| crate::proxy::https(headers))
    }
}

/// A cookie of the server's: `afterimage_<name>`, or `__Host-afterimage_<name>` over TLS.
#[derive(Debug, Clone, Copy)]
pub enum CookieName {
    Session,
    OidcFlow,
}

impl CookieName {
    #[must_use]
    pub fn with(self, secure: bool) -> &'static str {
        match (self, secure) {
            (Self::Session, false) => "afterimage_session",
            (Self::Session, true) => "__Host-afterimage_session",
            (Self::OidcFlow, false) => "afterimage_oidc",
            (Self::OidcFlow, true) => "__Host-afterimage_oidc",
        }
    }

    /// A `Set-Cookie` value holding `value` for `seconds`.
    #[must_use]
    pub fn set(self, value: String, seconds: i64, secure: bool) -> String {
        Cookie::build((self.with(secure), value))
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(secure)
            .max_age(cookie::time::Duration::seconds(seconds))
            .build()
            .to_string()
    }

    /// A `Set-Cookie` value that removes the cookie.
    #[must_use]
    pub fn clear(self, secure: bool) -> String {
        self.set(String::new(), 0, secure)
    }

    /// This cookie's value in a `Cookie` header.
    #[must_use]
    pub fn read(self, header: &str, secure: bool) -> Option<String> {
        Cookie::split_parse(header)
            .filter_map(Result::ok)
            .find(|cookie| cookie.name() == self.with(secure))
            .map(|cookie| cookie.value().to_owned())
            .filter(|value| !value.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookies_are_http_only_and_secure_over_tls() {
        let cookie = CookieName::Session.set("abc".into(), 60, true);
        assert!(
            cookie.starts_with("__Host-afterimage_session=abc"),
            "{cookie}"
        );
        for part in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/", "Max-Age=60"] {
            assert!(cookie.contains(part), "{cookie} lacks {part}");
        }
        let plain = CookieName::Session.set("abc".into(), 60, false);
        assert!(plain.starts_with("afterimage_session=abc") && !plain.contains("Secure"));
    }

    #[test]
    fn reads_only_its_own_cookie() {
        let header = "other=1; afterimage_session=token; __Host-afterimage_session=tls";
        assert_eq!(
            CookieName::Session.read(header, false).as_deref(),
            Some("token")
        );
        assert_eq!(
            CookieName::Session.read(header, true).as_deref(),
            Some("tls")
        );
        assert_eq!(CookieName::OidcFlow.read(header, false), None);
        assert_eq!(CookieName::Session.read("afterimage_session=", false), None);
    }

    #[test]
    fn the_public_url_decides_over_the_proxy() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert!(CookiePolicy::new(None).secure(&headers));
        assert!(!CookiePolicy::new(Some(false)).secure(&headers));
        assert!(!CookiePolicy::new(None).secure(&http::HeaderMap::new()));
    }
}
