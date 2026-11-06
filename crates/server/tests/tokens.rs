//! Personal API tokens: created by a signed-in person, then sent by scripts and
//! agents as `Authorization: Bearer <secret>` instead of a session.

mod common;

use common::{Server, mac, start};
use reqwest::StatusCode;
use serde_json::{Value, json};

/// A call with a token and no cookie, as a script makes it.
async fn with_token(
    server: &Server,
    secret: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let client = reqwest::Client::builder()
        .tls_certs_only([])
        .build()
        .unwrap();
    let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
    let mut request = client
        .request(method, format!("{}{path}", server.url))
        .bearer_auth(secret);
    if let Some(body) = body {
        request = request
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let bytes = response.bytes().await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Creates a token through the signed-in session and returns its id and secret.
async fn create(server: &Server, body: Value) -> (i64, String) {
    let (status, created) = server
        .call("POST", "/manage/account/tokens", Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    (
        created["token"]["id"].as_i64().unwrap(),
        created["secret"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn a_token_acts_as_its_owner_until_it_is_revoked() {
    let server = start().await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    let (id, secret) = create(&server, json!({ "name": "Provisioning" })).await;
    assert_eq!(secret.len(), 43, "32 random bytes in URL-safe base64");

    let (status, body) = with_token(&server, &secret, "GET", "/manage/devices", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["devices"].as_array().unwrap().len(), 1);
    let screen = json!({ "name": "From a script", "document": {
        "titleBar": { "title": "Hello" },
        "content": { "column": { "children": [{ "title": { "text": "Hi" } }] } } } });
    let (status, body) =
        with_token(&server, &secret, "POST", "/manage/screens", Some(screen)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (_, listed) = server.call("GET", "/manage/account/tokens", None).await;
    let token = &listed["tokens"][0];
    assert_eq!(token["name"], "Provisioning");
    assert!(token["lastUsedAt"].is_number(), "{token}");
    assert!(token.get("scope").is_none(), "the owner's whole reach");

    let (status, _) = server
        .call("DELETE", &format!("/manage/account/tokens/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = with_token(&server, &secret, "GET", "/manage/devices", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, _) = server
        .call("DELETE", &format!("/manage/account/tokens/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "already revoked");
}

#[tokio::test]
async fn a_read_only_token_reads_and_changes_nothing() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let (_, secret) = create(
        &server,
        json!({ "name": "Dashboard", "scope": "read_only" }),
    )
    .await;
    let (status, _) = with_token(
        &server,
        &secret,
        "GET",
        &format!("/manage/devices/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (method, path, body) in [
        (
            "PATCH",
            format!("/manage/devices/{id}"),
            Some(json!({ "name": "Renamed" })),
        ),
        ("DELETE", format!("/manage/devices/{id}"), None),
        ("POST", "/manage/auth/sign-out".to_owned(), None),
    ] {
        let (status, answer) = with_token(&server, &secret, method, &path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {answer}");
    }
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(body["device"]["name"], device["name"], "unchanged");
}

/// A token cannot mint or revoke one, so a leaked one cannot outlive its
/// revocation, and it has no session to end.
#[tokio::test]
async fn a_token_cannot_manage_tokens_or_sign_out() {
    let server = start().await;
    server.sign_up().await;
    let (id, secret) = create(&server, json!({ "name": "Agent" })).await;
    let (status, body) = with_token(
        &server,
        &secret,
        "POST",
        "/manage/account/tokens",
        Some(json!({ "name": "Another" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _) = with_token(
        &server,
        &secret,
        "DELETE",
        &format!("/manage/account/tokens/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = with_token(&server, &secret, "POST", "/manage/auth/sign-out", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = with_token(&server, &secret, "GET", "/manage/account/tokens", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["tokens"].as_array().unwrap().len(),
        1,
        "listing is reading"
    );
}

#[tokio::test]
async fn a_token_reaches_only_its_owners_resources() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let device = device["id"].as_i64().unwrap();
    let (status, created) = server
        .call(
            "POST",
            "/manage/screens",
            Some(json!({ "name": "Private", "document": {
                "titleBar": { "title": "Private" },
                "content": { "column": { "children": [{ "title": { "text": "Hi" } }] } } } })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let screen = created["screen"]["id"].as_i64().unwrap();

    // A second account with a token, written directly: only the OIDC provider
    // creates further accounts.
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    let eve: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (name, created_at) VALUES ('Eve', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let secret = "eve-token-secret-for-this-test-0000000000000";
    let hash = afterimage_server::keys::token_hash(secret);
    sqlx::query("INSERT INTO access_tokens (account_id, name, token_hash, created_at) VALUES (?, 'Eve', ?, 0)")
        .bind(eve)
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    let (status, _) = with_token(
        &server,
        secret,
        "GET",
        &format!("/manage/devices/{device}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = with_token(
        &server,
        secret,
        "GET",
        &format!("/manage/screens/{screen}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another account's screen");
    let (_, body) = with_token(&server, secret, "GET", "/manage/devices", None).await;
    assert_eq!(body["devices"], json!([]));
    let (status, _) = with_token(&server, secret, "GET", "/manage/account/tokens", None).await;
    assert_eq!(status, StatusCode::OK);
}

/// The secret appears in the answer that creates the token and nowhere after:
/// not in a listing, and not in the database, which holds its hash.
#[tokio::test]
async fn a_secret_is_shown_once_and_stored_as_a_hash() {
    let server = start().await;
    server.sign_up().await;
    let (_, secret) = create(&server, json!({ "name": "Once" })).await;
    let (_, listed) = server.call("GET", "/manage/account/tokens", None).await;
    assert!(!listed.to_string().contains(&secret));
    with_token(&server, &secret, "GET", "/manage/account", None).await;
    for path in server.database_files() {
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "{} holds the secret",
            path.display()
        );
    }
}

/// A request with both a token and a session acts with the token, so a script
/// run from a signed-in browser's cookie jar keeps the token's limits.
#[tokio::test]
async fn a_token_wins_over_a_session_cookie() {
    let server = start().await;
    server.sign_up().await;
    let (_, secret) = create(&server, json!({ "name": "Reader", "scope": "read_only" })).await;
    let response = server
        .http
        .post(format!("{}/manage/account/tokens", server.url))
        .bearer_auth(&secret)
        .header("Content-Type", "application/json")
        .body(json!({ "name": "Escalated" }).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// A bearer `Authorization` decides alone: a revoked or empty token is refused
/// even beside a valid session, and the scheme's name is case-insensitive (RFC
/// 9110, section 11.1). Another scheme leaves the session to decide.
#[tokio::test]
async fn the_authorization_header_decides_alone() {
    let server = start().await;
    server.sign_up().await;
    let (id, secret) = create(&server, json!({ "name": "Reader", "scope": "read_only" })).await;
    let creating = |authorization: String| {
        server
            .http
            .post(format!("{}/manage/account/tokens", server.url))
            .header("Authorization", authorization)
            .header("Content-Type", "application/json")
            .body(json!({ "name": "Escalated" }).to_string())
    };
    let response = creating(format!("bearer {secret}")).send().await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "a lowercase scheme is a bearer"
    );
    assert_eq!(response.headers()["x-amzn-errortype"], "Forbidden");
    let response = creating("Bearer ".to_owned()).send().await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "no fallback to the cookie"
    );
    assert_eq!(
        response.headers()["www-authenticate"],
        "Bearer error=\"invalid_token\""
    );
    // A reverse proxy's Basic credentials ride along with the browser's
    // requests; the session decides then.
    let response = creating("Basic YWRhOnNlY3JldA==".to_owned())
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "another scheme leaves it to the session"
    );

    server
        .call("DELETE", &format!("/manage/account/tokens/{id}"), None)
        .await;
    let response = server
        .http
        .get(format!("{}/manage/devices", server.url))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "a revoked token beside a session"
    );
    assert_eq!(response.headers()["x-amzn-errortype"], "Unauthenticated");
}
