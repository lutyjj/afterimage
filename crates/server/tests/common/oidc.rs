//! A stand-in OIDC provider: discovery, its published keys, and the
//! authorization and token endpoints of the authorization-code flow, as a
//! provider such as Pocket ID serves them. It signs RS256 ID tokens at the test's
//! clock for whoever the test signs in as, holds the client to what the flow asks
//! of it (its registered redirect, its secret, the PKCE verifier), and can flaw
//! its next answer in one chosen way.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Utc};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreIdTokenFields, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey, CoreSubjectIdentifierType,
    CoreTokenResponse, CoreTokenType,
};
use openidconnect::{
    AccessToken, Audience, AuthUrl, EmptyAdditionalClaims, EmptyAdditionalProviderMetadata,
    EmptyExtraTokenFields, EndUserName, EndUserUsername, IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey, ResponseTypes, Scope,
    StandardClaims, SubjectIdentifier, TokenUrl,
};
use afterimage_server::clock::Clock;
use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
use tokio::net::TcpListener;

use super::ManualClock;

pub const CLIENT_ID: &str = "afterimage";
pub const CLIENT_SECRET: &str = "client-secret";
/// The server's public URL, under which the provider sends browsers back.
pub const PUBLIC_URL: &str = "http://afterimage.example";
/// Where the server is registered to receive the provider's answer.
const CALLBACK: &str = "http://afterimage.example/manage/auth/oidc/callback";
/// What the sign-in button calls the provider.
pub const NAME: &str = "Example ID";

/// Two RSA keys in PKCS#1 PEM, made once per test binary: the provider signs
/// with one and publishes it, and can rotate to the other.
static KEYS: LazyLock<[String; 2]> = LazyLock::new(|| [new_key(), new_key()]);

fn new_key() -> String {
    rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048)
        .unwrap()
        .to_pkcs1_pem(LineEnding::LF)
        .unwrap()
        .to_string()
}

/// Key `index` of `KEYS` under the ID `kid`.
fn signing_key(index: usize, kid: usize) -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(&KEYS[index], Some(JsonWebKeyId::new(format!("key-{kid}"))))
        .unwrap()
}

/// Who the provider says signs in. It gives the name and username only to a
/// client that asked for the `profile` scope.
#[derive(Clone, Debug)]
pub struct Person {
    pub subject: String,
    pub name: Option<String>,
    pub username: Option<String>,
}

impl Person {
    pub fn named(subject: &str, name: &str) -> Self {
        Self {
            subject: subject.to_owned(),
            name: Some(name.to_owned()),
            username: None,
        }
    }
}

/// One way the provider's next answer to a code departs from a valid one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flaw {
    /// The ID token carries another nonce than the client sent.
    Nonce,
    /// It was issued for another client.
    Audience,
    /// It names another issuer.
    Issuer,
    /// It expired a second ago.
    Expired,
    /// A key the provider does not publish signed it, under the published key's ID.
    Signature,
    /// Its `at_hash` is the hash of another access token.
    AccessTokenHash,
    /// The answer holds no ID token.
    NoIdToken,
    /// The token endpoint refuses the code.
    InvalidGrant,
}

/// What the provider granted with a code, until the client exchanges it.
struct Grant {
    person: Person,
    nonce: String,
    challenge: String,
    profile: bool,
}

struct Inner {
    clock: Arc<ManualClock>,
    /// The index in `KEYS` of the key it signs with and publishes.
    key: usize,
    person: Person,
    flaw: Option<Flaw>,
    grants: HashMap<String, Grant>,
    codes_issued: usize,
}

#[derive(Clone)]
pub struct Provider {
    /// The issuer, which is also where it serves.
    pub url: String,
    inner: Arc<Mutex<Inner>>,
}

impl Provider {
    /// A provider on a free local port, at `clock`'s time.
    pub async fn start(clock: Arc<ManualClock>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let provider = Self {
            url: format!("http://{}", listener.local_addr().unwrap()),
            inner: Arc::new(Mutex::new(Inner {
                clock,
                key: 0,
                person: Person::named("subject-1", "Grace"),
                flaw: None,
                grants: HashMap::new(),
                codes_issued: 0,
            })),
        };
        let app = axum::Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(keys))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .with_state(provider.clone());
        tokio::spawn(async move { axum::serve(listener, app).await });
        provider
    }

    /// The server's settings for signing in here.
    pub fn settings(&self) -> Vec<String> {
        [
            "--oidc-issuer",
            &self.url,
            "--oidc-client-id",
            CLIENT_ID,
            "--oidc-client-secret",
            CLIENT_SECRET,
            "--oidc-name",
            NAME,
            "--public-url",
            PUBLIC_URL,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// Who signs in at the next authorization.
    pub fn sign_in_as(&self, person: Person) {
        self.inner.lock().unwrap().person = person;
    }

    /// Flaws the next answer to a code.
    pub fn flaw_next(&self, flaw: Flaw) {
        self.inner.lock().unwrap().flaw = Some(flaw);
    }

    /// Signs with the other key from now on and publishes only that one.
    pub fn rotate_key(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.key = 1 - inner.key;
    }
}

async fn discovery(State(provider): State<Provider>) -> Response {
    let url = &provider.url;
    let metadata = CoreProviderMetadata::new(
        IssuerUrl::new(url.clone()).unwrap(),
        AuthUrl::new(format!("{url}/authorize")).unwrap(),
        JsonWebKeySetUrl::new(format!("{url}/jwks")).unwrap(),
        vec![ResponseTypes::new(vec![CoreResponseType::Code])],
        vec![CoreSubjectIdentifierType::Public],
        vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
        EmptyAdditionalProviderMetadata {},
    )
    .set_token_endpoint(Some(TokenUrl::new(format!("{url}/token")).unwrap()))
    .set_scopes_supported(Some(vec![
        Scope::new("openid".to_owned()),
        Scope::new("profile".to_owned()),
    ]));
    axum::Json(metadata).into_response()
}

async fn keys(State(provider): State<Provider>) -> axum::Json<CoreJsonWebKeySet> {
    let key = provider.inner.lock().unwrap().key;
    axum::Json(CoreJsonWebKeySet::new(vec![
        signing_key(key, key).as_verification_key(),
    ]))
}

/// Grants a code to whoever the test signs in as and sends the browser back,
/// provided the request is the one the client is registered for.
async fn authorize(
    State(provider): State<Provider>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let field = |name: &str| query.get(name).map(String::as_str);
    let scopes: Vec<&str> = field("scope").unwrap_or_default().split(' ').collect();
    let registered = field("response_type") == Some("code")
        && field("client_id") == Some(CLIENT_ID)
        && field("redirect_uri") == Some(CALLBACK)
        && scopes.contains(&"openid")
        && field("code_challenge_method") == Some("S256");
    let (true, Some(state), Some(nonce), Some(challenge)) = (
        registered,
        field("state"),
        field("nonce"),
        field("code_challenge"),
    ) else {
        return (
            StatusCode::BAD_REQUEST,
            "not a request this client may make",
        )
            .into_response();
    };
    let mut inner = provider.inner.lock().unwrap();
    inner.codes_issued += 1;
    let code = format!("code-{}", inner.codes_issued);
    let grant = Grant {
        person: inner.person.clone(),
        nonce: nonce.to_owned(),
        challenge: challenge.to_owned(),
        profile: scopes.contains(&"profile"),
    };
    inner.grants.insert(code.clone(), grant);
    let mut back = url::Url::parse(CALLBACK).unwrap();
    back.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", state);
    (StatusCode::FOUND, [(header::LOCATION, back.to_string())]).into_response()
}

/// Exchanges a code for tokens, once, for the client that authenticates with its
/// secret and proves the PKCE challenge.
async fn token(
    State(provider): State<Provider>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let refuse = |status: StatusCode, error: &str| {
        (status, axum::Json(serde_json::json!({ "error": error }))).into_response()
    };
    let credentials = format!(
        "Basic {}",
        STANDARD.encode(format!("{CLIENT_ID}:{CLIENT_SECRET}"))
    );
    if headers
        .get(header::AUTHORIZATION)
        .map(axum::http::HeaderValue::as_bytes)
        != Some(credentials.as_bytes())
    {
        return refuse(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let field = |name: &str| form.get(name).map(String::as_str);
    let mut inner = provider.inner.lock().unwrap();
    let flaw = inner.flaw.take();
    let Some(code) = field("code") else {
        return refuse(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Some(grant) = inner.grants.remove(code) else {
        return refuse(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let proven = field("code_verifier").is_some_and(|verifier| {
        PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(verifier.to_owned()))
            .as_str()
            == grant.challenge
    });
    if field("grant_type") != Some("authorization_code")
        || field("redirect_uri") != Some(CALLBACK)
        || !proven
        || flaw == Some(Flaw::InvalidGrant)
    {
        return refuse(StatusCode::BAD_REQUEST, "invalid_grant");
    }

    let now = inner.clock.now();
    let at = |seconds: i64| DateTime::<Utc>::from_timestamp(seconds, 0).unwrap();
    let mut person = StandardClaims::new(SubjectIdentifier::new(grant.person.subject));
    if grant.profile {
        person = person
            .set_name(grant.person.name.map(|name| EndUserName::new(name).into()))
            .set_preferred_username(grant.person.username.map(EndUserUsername::new));
    }
    let issuer = match flaw {
        Some(Flaw::Issuer) => format!("{}/another", provider.url),
        _ => provider.url.clone(),
    };
    let audience = match flaw {
        Some(Flaw::Audience) => "another-client",
        _ => CLIENT_ID,
    };
    let expires = match flaw {
        Some(Flaw::Expired) => now - 1,
        _ => now + 300,
    };
    let nonce = match flaw {
        Some(Flaw::Nonce) => "another-nonce".to_owned(),
        _ => grant.nonce,
    };
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(issuer).unwrap(),
        vec![Audience::new(audience.to_owned())],
        at(expires),
        at(now),
        person,
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(nonce)));
    let key = match flaw {
        Some(Flaw::Signature) => signing_key(1 - inner.key, inner.key),
        _ => signing_key(inner.key, inner.key),
    };
    let access = AccessToken::new(format!("access-{code}"));
    let hashed = match flaw {
        Some(Flaw::AccessTokenHash) => AccessToken::new("another access token".to_owned()),
        _ => access.clone(),
    };
    let id_token = CoreIdToken::new(
        claims,
        &key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&hashed),
        None,
    )
    .unwrap();
    let id_token = (flaw != Some(Flaw::NoIdToken)).then_some(id_token);
    let mut answer = CoreTokenResponse::new(
        access,
        CoreTokenType::Bearer,
        CoreIdTokenFields::new(id_token, EmptyExtraTokenFields {}),
    );
    answer.set_expires_in(Some(&Duration::from_secs(300)));
    axum::Json(answer).into_response()
}
