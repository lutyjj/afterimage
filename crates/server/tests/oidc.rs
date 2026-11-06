//! Signing in through an OIDC provider, against a stand-in provider that signs
//! its ID tokens at the server's time.

mod common;

use std::sync::Arc;

use common::oidc::{Flaw, NAME, PUBLIC_URL, Person};
use common::{Server, start, start_with, start_with_provider};
use afterimage_server::keys::OsEntropy;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Where the server sends a browser that signed in.
const LANDING: &str = "http://afterimage.example/";
const REFUSED: &str = "http://afterimage.example/?signInError=refused";

/// A browser of its own: it keeps cookies and follows a redirect only when the
/// test does, and it reaches the server under test at the server's public URL.
struct Browser {
    http: reqwest::Client,
}

impl Browser {
    fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .tls_certs_only([])
                .cookie_store(true)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }

    async fn get(&self, server: &Server, location: &str) -> reqwest::Response {
        let url = location.replacen(PUBLIC_URL, &server.url, 1);
        self.http.get(url).send().await.unwrap()
    }

    /// Starts a sign-in and lets the provider answer it; returns the provider's
    /// way back to the server, with its code and the flow's state.
    async fn authorize(&self, server: &Server) -> String {
        let start = self
            .get(server, &format!("{PUBLIC_URL}/manage/auth/oidc"))
            .await;
        assert_eq!(start.status(), StatusCode::FOUND);
        let at_provider = self.get(server, location(&start)).await;
        assert_eq!(at_provider.status(), StatusCode::FOUND);
        location(&at_provider).to_owned()
    }

    /// Follows the provider's way back; returns where the server sends the
    /// browser then.
    async fn finish(&self, server: &Server, callback: &str) -> String {
        let back = self.get(server, callback).await;
        assert_eq!(back.status(), StatusCode::FOUND);
        location(&back).to_owned()
    }

    async fn sign_in(&self, server: &Server) -> String {
        let callback = self.authorize(server).await;
        self.finish(server, &callback).await
    }

    /// The account this browser's session belongs to, if it has one.
    async fn account(&self, server: &Server) -> Option<Value> {
        let response = self
            .get(server, &format!("{PUBLIC_URL}/manage/account"))
            .await;
        match response.status() {
            StatusCode::OK => Some(response.json::<Value>().await.unwrap()["account"].clone()),
            StatusCode::UNAUTHORIZED => None,
            status => panic!("reading the account: {status}"),
        }
    }
}

fn location(response: &reqwest::Response) -> &str {
    response.headers()["location"].to_str().unwrap()
}

async fn accounts(server: &Server) -> usize {
    server.stored("SELECT id FROM accounts").await.len()
}

#[tokio::test]
async fn oidc_sign_in_is_absent_until_configured() {
    let server = start().await;
    assert!(
        server
            .call("GET", "/manage/auth", None)
            .await
            .1
            .get("oidc")
            .is_none()
    );
    assert_eq!(
        server.call("GET", "/manage/auth/oidc", None).await.0,
        StatusCode::NOT_FOUND
    );
}

/// The provider's person signs in to the account of their issuer and subject,
/// made at their first sign-in and the same at every later one.
#[tokio::test]
async fn a_sign_in_through_the_provider_starts_a_session_for_its_account() {
    let (server, provider) = start_with_provider(&[]).await;
    let (_, options) = server.call("GET", "/manage/auth", None).await;
    assert_eq!(options["oidc"]["name"], NAME);

    let grace = Browser::new();
    assert_eq!(grace.sign_in(&server).await, LANDING);
    let account = grace.account(&server).await.expect("a session");
    assert_eq!(account["name"], "Grace");
    assert!(account.get("username").is_none(), "{account}");

    let again = Browser::new();
    again.sign_in(&server).await;
    assert_eq!(again.account(&server).await.unwrap()["id"], account["id"]);

    provider.sign_in_as(Person::named("subject-2", "Ada"));
    let ada = Browser::new();
    ada.sign_in(&server).await;
    let other = ada.account(&server).await.unwrap();
    assert_ne!(other["id"], account["id"]);
    assert_eq!(other["name"], "Ada");
    assert_eq!(accounts(&server).await, 2);
}

/// An account is named from the profile the provider shares: the name, else the
/// username, else a placeholder; trimmed, at most 64 characters.
#[tokio::test]
async fn an_account_is_named_from_the_profile() {
    let (server, provider) = start_with_provider(&[]).await;
    let long = "A".repeat(70);
    let cut_at_a_space = format!("{} Hopper", "A".repeat(63));
    for (index, (name, username, expected)) in [
        (Some("Grace Hopper"), Some("grace"), "Grace Hopper"),
        (None, Some("grace"), "grace"),
        (Some("  "), Some("grace"), "grace"),
        (Some(" Grace "), None, "Grace"),
        (None, None, "Account"),
        (Some(long.as_str()), None, &long[..64]),
        (Some(cut_at_a_space.as_str()), None, &long[..63]),
    ]
    .into_iter()
    .enumerate()
    {
        provider.sign_in_as(Person {
            subject: format!("subject-{index}"),
            name: name.map(str::to_owned),
            username: username.map(str::to_owned),
        });
        let browser = Browser::new();
        browser.sign_in(&server).await;
        assert_eq!(
            browser.account(&server).await.unwrap()["name"],
            expected,
            "{name:?} {username:?}"
        );
    }
}

/// An answer from the provider that does not verify signs no one in and makes
/// no account.
#[tokio::test]
async fn an_answer_that_does_not_verify_signs_no_one_in() {
    let (server, provider) = start_with_provider(&[]).await;
    for flaw in [
        Flaw::Nonce,
        Flaw::Audience,
        Flaw::Issuer,
        Flaw::Expired,
        Flaw::Signature,
        Flaw::AccessTokenHash,
        Flaw::NoIdToken,
        Flaw::InvalidGrant,
    ] {
        provider.flaw_next(flaw);
        let browser = Browser::new();
        assert_eq!(browser.sign_in(&server).await, REFUSED, "{flaw:?}");
        assert!(browser.account(&server).await.is_none(), "{flaw:?}");
    }
    provider.sign_in_as(Person::named(&"s".repeat(256), "Grace"));
    let browser = Browser::new();
    assert_eq!(
        browser.sign_in(&server).await,
        REFUSED,
        "a subject too long"
    );
    assert_eq!(accounts(&server).await, 0);
}

/// A sign-in finishes only in the browser that started it, with the state it
/// was given, once.
#[tokio::test]
async fn a_sign_in_finishes_once_in_the_browser_that_started_it() {
    let (server, _provider) = start_with_provider(&[]).await;
    let grace = Browser::new();
    let mallory = Browser::new();

    // The provider's code stays unused through these three, so each is refused
    // for the one reason it names.
    let callback = grace.authorize(&server).await;
    assert_eq!(
        mallory.finish(&server, &callback).await,
        REFUSED,
        "a browser with no flow"
    );
    let state = callback.split("state=").nth(1).unwrap();
    let altered = callback.replace(state, "another-state");
    assert_eq!(
        grace.finish(&server, &altered).await,
        REFUSED,
        "another state"
    );
    assert_eq!(
        grace.finish(&server, &callback).await,
        REFUSED,
        "a flow that attempt used up"
    );
    assert!(grace.account(&server).await.is_none());

    let callback = grace.authorize(&server).await;
    mallory.authorize(&server).await;
    assert_eq!(
        mallory.finish(&server, &callback).await,
        REFUSED,
        "another browser's flow"
    );
    assert!(mallory.account(&server).await.is_none());

    let callback = grace.authorize(&server).await;
    assert_eq!(grace.finish(&server, &callback).await, LANDING);
    assert!(grace.account(&server).await.is_some());
    assert_eq!(grace.finish(&server, &callback).await, REFUSED, "a replay");
}

/// A browser has ten minutes to come back from the provider.
#[tokio::test]
async fn a_sign_in_expires_after_ten_minutes() {
    let (server, _provider) = start_with_provider(&[]).await;
    let late = Browser::new();
    let callback = late.authorize(&server).await;
    server.clock.advance(10 * 60);
    assert_eq!(late.finish(&server, &callback).await, REFUSED);

    let in_time = Browser::new();
    let callback = in_time.authorize(&server).await;
    server.clock.advance(10 * 60 - 1);
    assert_eq!(in_time.finish(&server, &callback).await, LANDING);
}

/// The provider's keys are read at each sign-in, so a rotation needs no restart.
#[tokio::test]
async fn the_provider_may_rotate_its_key() {
    let (server, provider) = start_with_provider(&[]).await;
    assert_eq!(Browser::new().sign_in(&server).await, LANDING);
    provider.rotate_key();
    let browser = Browser::new();
    assert_eq!(browser.sign_in(&server).await, LANDING);
    assert!(browser.account(&server).await.is_some());
}

/// With password sign-in off, no password signs anyone in and the provider does.
#[tokio::test]
async fn with_password_sign_in_off_only_the_provider_signs_in() {
    let (server, _provider) = start_with_provider(&["--password-sign-in", "false"]).await;
    let (_, options) = server.call("GET", "/manage/auth", None).await;
    assert_eq!(options["password"], false);
    let account = json!({ "name": "Ada", "username": "ada", "password": "correct horse battery" });
    assert_eq!(
        server
            .call("POST", "/manage/auth/first-account", Some(account))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let credentials = json!({ "username": "ada", "password": "correct horse battery" });
    assert_eq!(
        server
            .call("POST", "/manage/auth/password", Some(credentials))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let browser = Browser::new();
    assert_eq!(browser.sign_in(&server).await, LANDING);
    assert!(browser.account(&server).await.is_some());
}

/// A sign-in that cannot finish because the provider is out of reach sends the
/// browser back to the web UI with the reason, and starts no session.
#[tokio::test]
async fn a_sign_in_with_the_provider_out_of_reach_returns_to_the_web_ui() {
    let settings = [
        "--oidc-issuer",
        "http://127.0.0.1:9",
        "--oidc-client-id",
        "afterimage",
        "--oidc-client-secret",
        "secret",
        "--oidc-name",
        NAME,
        "--public-url",
        PUBLIC_URL,
    ];
    let server = start_with(Arc::new(OsEntropy), &settings).await;
    let browser = Browser::new();
    let started = browser
        .get(&server, &format!("{PUBLIC_URL}/manage/auth/oidc"))
        .await;
    assert_eq!(started.status(), StatusCode::FOUND);
    assert_eq!(
        location(&started),
        "http://afterimage.example/?signInError=unavailable"
    );
    assert!(started.headers().get("set-cookie").is_none());

    for query in ["?code=c&state=s", "?error=access_denied", ""] {
        let back = browser
            .finish(
                &server,
                &format!("{PUBLIC_URL}/manage/auth/oidc/callback{query}"),
            )
            .await;
        assert_eq!(back, REFUSED, "{query}");
    }
    assert!(browser.account(&server).await.is_none());
    assert_eq!(accounts(&server).await, 0);
}
