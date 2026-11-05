//! Sign-in through an OIDC provider: the authorization-code flow with PKCE and a
//! nonce. A verified issuer and subject identify an account; the provider's tokens
//! are never kept.
//!
//! Between the redirect to the provider and its answer, the flow's state, nonce and
//! PKCE verifier wait in the database under the hash of a cookie that binds them to
//! the browser that started it; the answer consumes them once.

use std::sync::Arc;
use std::time::Duration;

use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, reqwest,
};
use sqlx::SqlitePool;

use crate::clock::Clock;
use crate::keys::{self, Entropy};
use crate::sessions::CookieName;

/// How long a browser has to come back from the provider.
pub const FLOW_SECONDS: i64 = 10 * 60;
/// Where the provider sends the browser back, under the public URL.
pub const CALLBACK_PATH: &str = "/manage/auth/oidc/callback";

/// The provider, from the deployment's settings.
#[derive(Debug, Clone)]
pub struct Settings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// What the sign-in button calls the provider.
    pub name: String,
}

type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the OIDC provider could not be reached or described itself wrongly")]
    Unavailable,
    #[error("the OIDC provider's answer could not be verified")]
    Refused,
    #[error("sign-in storage: {0}")]
    Storage(#[from] sqlx::Error),
}

pub struct Started {
    /// The provider's authorization URL.
    pub location: String,
    /// The `Set-Cookie` value that binds the flow to the browser.
    pub cookie: String,
}

/// Who the provider says signed in.
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub name: String,
}

pub struct Oidc {
    issuer: IssuerUrl,
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect: RedirectUrl,
    name: String,
    http: reqwest::Client,
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    entropy: Arc<dyn Entropy>,
}

impl Oidc {
    /// # Errors
    ///
    /// When the issuer or the public URL is not a URL.
    pub fn new(
        settings: Settings,
        public_url: &str,
        pool: SqlitePool,
        clock: Arc<dyn Clock>,
        entropy: Arc<dyn Entropy>,
    ) -> Result<Self, String> {
        let issuer = IssuerUrl::new(settings.issuer)
            .map_err(|error| format!("the OIDC issuer is not a URL: {error}"))?;
        let redirect = RedirectUrl::new(format!(
            "{}{CALLBACK_PATH}",
            public_url.trim_end_matches('/')
        ))
        .map_err(|error| format!("the public URL is not a URL: {error}"))?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("the OIDC HTTP client: {error}"))?;
        Ok(Self {
            issuer,
            client_id: ClientId::new(settings.client_id),
            client_secret: ClientSecret::new(settings.client_secret),
            redirect,
            name: settings.name,
            http,
            pool,
            clock,
            entropy,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Discovered on every flow, so the provider's key rotation needs no restart.
    async fn client(&self) -> Result<DiscoveredClient, Error> {
        let metadata = CoreProviderMetadata::discover_async(self.issuer.clone(), &self.http)
            .await
            .map_err(|error| {
                tracing::warn!(%error, "OIDC discovery failed");
                Error::Unavailable
            })?;
        Ok(CoreClient::from_provider_metadata(
            metadata,
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect.clone()))
    }

    /// Begins a sign-in: where to send the browser, and the cookie that binds the
    /// flow to it.
    ///
    /// # Errors
    ///
    /// When the provider is unavailable or storage fails.
    pub async fn start(&self, secure: bool) -> Result<Started, Error> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        let binding = keys::new_token(&*self.entropy);
        let binding_hash = keys::token_hash(&binding);
        let now = self.clock.now();
        let expires_at = now + FLOW_SECONDS;
        let (state, nonce, verifier) = (state.secret(), nonce.secret(), verifier.secret());
        sqlx::query!("DELETE FROM oidc_flows WHERE expires_at <= ?", now)
            .execute(&self.pool)
            .await?;
        sqlx::query!(
            "INSERT INTO oidc_flows (binding_hash, state, nonce, pkce_verifier, expires_at)
             VALUES (?, ?, ?, ?, ?)",
            binding_hash,
            state,
            nonce,
            verifier,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(Started {
            location: url.to_string(),
            cookie: CookieName::OidcFlow.set(binding, FLOW_SECONDS, secure),
        })
    }

    /// Finishes a sign-in with the provider's `code` and `state` and the browser's
    /// binding cookie value.
    ///
    /// # Errors
    ///
    /// `Refused` when the flow is unknown, expired or bound to another browser, or
    /// the provider's answer does not verify; `Unavailable` when the provider cannot
    /// be reached.
    pub async fn complete(
        &self,
        code: String,
        state: &str,
        binding: &str,
    ) -> Result<Identity, Error> {
        let binding_hash = keys::token_hash(binding);
        let now = self.clock.now();
        let flow = sqlx::query!(
            "DELETE FROM oidc_flows WHERE binding_hash = ? AND expires_at > ?
             RETURNING state, nonce, pkce_verifier",
            binding_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::Refused)?;
        if flow.state != state {
            return Err(Error::Refused);
        }
        let client = self.client().await?;
        let tokens = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|_| Error::Unavailable)?
            .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier))
            .request_async(&self.http)
            .await
            .map_err(|error| {
                tracing::warn!(%error, "OIDC code exchange failed");
                Error::Refused
            })?;
        let token = tokens.id_token().ok_or(Error::Refused)?;
        let verifier = client.id_token_verifier();
        let claims = token
            .claims(&verifier, &Nonce::new(flow.nonce))
            .map_err(|_| Error::Refused)?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                tokens.access_token(),
                token.signing_alg().map_err(|_| Error::Refused)?,
                token.signing_key(&verifier).map_err(|_| Error::Refused)?,
            )
            .map_err(|_| Error::Refused)?;
            if &actual != expected {
                return Err(Error::Refused);
            }
        }
        let subject = claims.subject().as_str();
        if subject.is_empty() || subject.len() > 255 {
            return Err(Error::Refused);
        }
        let name = claims
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.as_str().to_owned())
            .or_else(|| {
                claims
                    .preferred_username()
                    .map(|name| name.as_str().to_owned())
            })
            .map(|name| name.chars().take(64).collect::<String>())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Account".to_owned());
        Ok(Identity {
            issuer: self.issuer.as_str().to_owned(),
            subject: subject.to_owned(),
            name,
        })
    }
}
