//! The MQTT broker set in the web UI: only the administrator, the first account,
//! reads and changes it; it takes effect at once, moving or clearing what the
//! server published; the password is written and never read back; and where the
//! deployment sets the broker, it cannot change here.

mod common;

use std::sync::Arc;

use common::{Broker, Observer, Server, mac, start, start_on_data, start_with};
use afterimage_server::keys::OsEntropy;
use reqwest::StatusCode;
use serde_json::{Value, json};

const SETTINGS: &str = "/manage/server/mqtt";
const CONFIG: &str = "homeassistant/device/afterimage_41b41039a101/config";
const MOVED: &str = "homeassistant/device/frames_41b41039a101/config";

async fn settings(server: &Server) -> Value {
    let (status, body) = server.call("GET", SETTINGS, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["settings"].clone()
}

async fn save(server: &Server, settings: Value) -> Value {
    let (status, body) = server.call("PUT", SETTINGS, Some(settings)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["settings"].clone()
}

/// Waits until the server reports `connection`.
async fn connection(server: &Server, connection: &str) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while settings(server).await["connection"] != connection {
        assert!(tokio::time::Instant::now() < deadline, "never {connection}");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// A second account, which only the OIDC provider creates, written directly,
/// signed in with a password on a client of its own.
async fn second_account(server: &Server) -> Server {
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO accounts (name, username, password_hash, created_at) VALUES ('Eve', 'eve', ?, 0)",
    )
    .bind(password_auth::generate_hash("second password here"))
    .execute(&pool)
    .await
    .unwrap();
    let eve = Server {
        http: reqwest::Client::builder()
            .tls_certs_only([])
            .cookie_store(true)
            .build()
            .unwrap(),
        ..server.clone_handle()
    };
    let credentials = json!({ "username": "eve", "password": "second password here" });
    let (status, _) = eve
        .call("POST", "/manage/auth/password", Some(credentials))
        .await;
    assert_eq!(status, StatusCode::OK);
    eve
}

#[tokio::test]
async fn only_the_first_account_administers_the_server() {
    let server = start().await;
    let first = server.sign_up().await;
    assert_eq!(first["administrator"], true);
    assert_eq!(
        server.call("GET", "/manage/account", None).await.1["account"]["administrator"],
        true
    );
    let defaults = settings(&server).await;
    assert_eq!(
        defaults,
        json!({
            "setByDeployment": false,
            "passwordSet": false,
            "topic": "afterimage",
            "discoveryPrefix": "homeassistant",
            "connection": "off"
        })
    );

    let eve = second_account(&server).await;
    assert_eq!(
        eve.call("GET", "/manage/account", None).await.1["account"]["administrator"],
        false
    );
    assert_eq!(
        eve.call("GET", SETTINGS, None).await.0,
        StatusCode::FORBIDDEN
    );
    let broker = json!({ "url": "mqtt://broker.example.test:1883" });
    assert_eq!(
        eve.call("PUT", SETTINGS, Some(broker)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(settings(&server).await, defaults, "nothing changed");
}

#[tokio::test]
async fn a_saved_broker_takes_effect_at_once_and_moves_or_clears_the_topics() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let server = start().await;
    server.sign_up().await;
    server.claim(&mac(1)).await;

    let saved = save(&server, json!({ "url": broker.url() })).await;
    assert_eq!(saved["url"], broker.url());
    observer.wait(CONFIG, |_| true).await;
    assert_eq!(observer.wait("afterimage/status", |_| true).await, "online");
    connection(&server, "connected").await;

    // Another topic: what was published under the first goes, and the device
    // appears under the second.
    save(&server, json!({ "url": broker.url(), "topic": "frames" })).await;
    observer.wait(MOVED, |_| true).await;
    assert_eq!(observer.wait("frames/status", |_| true).await, "online");
    observer.wait_gone(CONFIG).await;
    observer.wait_gone("afterimage/status").await;
    observer.wait_gone("afterimage/41b41039a101/state").await;

    // No broker: everything published goes, and the server publishes nowhere.
    let off = save(&server, json!({})).await;
    assert_eq!(off["connection"], "off");
    assert!(off.get("url").is_none(), "{off}");
    observer.wait_gone(MOVED).await;
    observer.wait_gone("frames/status").await;

    // Each connection said goodbye, so the broker never published a last will.
    for status in ["afterimage/status", "frames/status"] {
        let payloads = observer.payloads(status);
        assert!(
            !payloads.contains(&"offline".to_owned()),
            "{status}: {payloads:?}"
        );
    }
}

#[tokio::test]
async fn other_credentials_or_the_same_settings_keep_what_was_published() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let server = start().await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    let settings = json!({ "url": broker.url(), "username": "afterimage", "password": "first" });
    save(&server, settings.clone()).await;
    observer.wait(CONFIG, |_| true).await;
    connection(&server, "connected").await;

    save(&server, settings).await;
    let rotated = json!({ "url": broker.url(), "username": "afterimage", "password": "second" });
    save(&server, rotated).await;
    connection(&server, "connected").await;
    observer.settle(CONFIG).await;
    assert!(
        !observer.payloads(CONFIG).iter().any(String::is_empty),
        "the device never left Home Assistant"
    );
}

#[tokio::test]
async fn a_saved_password_goes_only_to_its_own_broker_and_username() {
    let broker = Broker::start().await;
    let server = start().await;
    server.sign_up().await;
    let url = broker.url();
    save(
        &server,
        json!({ "url": url, "username": "afterimage", "password": "secret" }),
    )
    .await;

    let refused = |settings: Value| {
        let server = &server;
        async move { server.call("PUT", SETTINGS, Some(settings)).await }
    };
    // Another host, or another username, without the password again.
    let (status, body) =
        refused(json!({ "url": "mqtt://other.example.test:1883", "username": "afterimage" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _) = refused(json!({ "url": url, "username": "someone-else" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(settings(&server).await["url"], url, "nothing changed");

    // The same broker, spelt with a slash, keeps it.
    let kept = save(
        &server,
        json!({ "url": format!("{url}/"), "username": "afterimage" }),
    )
    .await;
    assert_eq!(kept["passwordSet"], true);
}

#[tokio::test]
async fn a_change_while_the_broker_is_away_does_not_wait_for_it() {
    let broker = Broker::start().await;
    let server = start().await;
    server.sign_up().await;
    save(&server, json!({ "url": broker.url() })).await;
    connection(&server, "connected").await;
    let url = broker.url();
    drop(broker);
    connection(&server, "connecting").await;

    let started = std::time::Instant::now();
    let moved = save(&server, json!({ "url": url, "topic": "frames" })).await;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(7),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(moved["topic"], "frames");
    assert_eq!(moved["connection"], "connecting");
}

#[tokio::test]
async fn a_broker_that_refuses_shows_as_refused() {
    let broker = Broker::start_refusing().await;
    let server = start().await;
    server.sign_up().await;
    save(&server, json!({ "url": broker.url() })).await;
    connection(&server, "refused").await;
}

#[tokio::test]
async fn tokens_act_as_their_account_on_the_broker_settings() {
    let server = start().await;
    server.sign_up().await;
    let token = |server: &Server, scope: Value| {
        let server = server.clone_handle();
        async move {
            let mut body = json!({ "name": "Script" });
            if !scope.is_null() {
                body["scope"] = scope;
            }
            let (status, created) = server
                .call("POST", "/manage/account/tokens", Some(body))
                .await;
            assert_eq!(status, StatusCode::CREATED, "{created}");
            created["secret"].as_str().unwrap().to_owned()
        }
    };
    let with = |secret: String, method: &'static str, body: Option<Value>| {
        let url = format!("{}{SETTINGS}", server.url);
        async move {
            // No CA store: the Nix sandbox has none, and the server is plain HTTP.
            let client = reqwest::Client::builder()
                .tls_certs_only([])
                .build()
                .unwrap();
            let mut request = client
                .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), url)
                .bearer_auth(secret);
            if let Some(body) = body {
                request = request.json(&body);
            }
            request.send().await.unwrap().status()
        }
    };
    let read_only = token(&server, json!("read_only")).await;
    assert_eq!(with(read_only.clone(), "GET", None).await, StatusCode::OK);
    assert_eq!(
        with(read_only, "PUT", Some(json!({}))).await,
        StatusCode::FORBIDDEN
    );
    let eve = second_account(&server).await;
    let members = token(&eve, Value::Null).await;
    assert_eq!(with(members, "GET", None).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_first_account_of_an_older_database_administers_it() {
    // A database at the schema the deployments ran before administrators, with
    // two accounts: the migration names the first.
    let data = tempfile::TempDir::new().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data.path().join("afterimage.db"))
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    let mut earlier = sqlx::migrate!("./migrations");
    earlier.migrations = earlier
        .migrations
        .iter()
        .filter(|migration| migration.version <= 3)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    earlier.run(&pool).await.unwrap();
    let hash = password_auth::generate_hash("correct horse battery");
    for (name, username) in [("Ada", "ada"), ("Eve", "eve")] {
        sqlx::query(
            "INSERT INTO accounts (name, username, password_hash, created_at) VALUES (?, ?, ?, 0)",
        )
        .bind(name)
        .bind(username)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool.close().await;

    let server = start_on_data(data).await;
    for (username, administrator) in [("ada", true), ("eve", false)] {
        let client = Server {
            http: reqwest::Client::builder()
                .tls_certs_only([])
                .cookie_store(true)
                .build()
                .unwrap(),
            ..server.clone_handle()
        };
        let credentials = json!({ "username": username, "password": "correct horse battery" });
        let (status, body) = client
            .call("POST", "/manage/auth/password", Some(credentials))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["account"]["administrator"], administrator,
            "{username}"
        );
    }
}

#[tokio::test]
async fn a_saved_broker_holds_after_a_restart() {
    let broker = Broker::start().await;
    let observer = Observer::start(&broker);
    let server = start().await;
    server.sign_up().await;
    server.claim(&mac(1)).await;
    save(&server, json!({ "url": broker.url(), "topic": "frames" })).await;
    observer.wait(MOVED, |_| true).await;
    // The first server lets go of the broker, so only the restarted one holds it;
    // saving no broker forgets the settings, so they are written back directly.
    save(&server, json!({})).await;
    observer.wait_gone(MOVED).await;
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO mqtt_settings (id, url, topic, discovery_prefix, updated_at)
         VALUES (1, ?, 'frames', 'homeassistant', 0)",
    )
    .bind(broker.url())
    .execute(&pool)
    .await
    .unwrap();

    let restarted = server.again(&[]).await;
    observer.wait(MOVED, |_| true).await;
    let credentials = json!({ "username": "ada", "password": "correct horse battery" });
    restarted
        .call("POST", "/manage/auth/password", Some(credentials))
        .await;
    connection(&restarted, "connected").await;
}

#[tokio::test]
async fn the_password_is_written_and_never_read() {
    let broker = Broker::start().await;
    let server = start().await;
    server.sign_up().await;
    let secret = "a broker password of the test's own";
    let with = |password: Option<&str>| {
        let mut settings = json!({ "url": broker.url(), "username": "afterimage" });
        if let Some(password) = password {
            settings["password"] = json!(password);
        }
        settings
    };

    let saved = save(&server, with(Some(secret))).await;
    assert_eq!(saved["passwordSet"], true);
    assert_eq!(saved["username"], "afterimage");
    let (_, body) = server.call("GET", SETTINGS, None).await;
    assert!(!body.to_string().contains(secret), "{body}");

    // Absent keeps it; empty removes it.
    assert_eq!(save(&server, with(None)).await["passwordSet"], true);
    assert_eq!(save(&server, with(Some(""))).await["passwordSet"], false);

    let without_username = json!({ "url": broker.url(), "password": secret });
    assert_eq!(
        server.call("PUT", SETTINGS, Some(without_username)).await.0,
        StatusCode::BAD_REQUEST
    );
    for url in [
        "mqtt://",
        "mqtt://broker.example.test/path",
        "http://broker.example.test",
    ] {
        assert_eq!(
            server
                .call("PUT", SETTINGS, Some(json!({ "url": url })))
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{url}"
        );
    }
}

#[tokio::test]
async fn the_deployments_broker_is_shown_and_cannot_change_here() {
    let broker = Broker::start().await;
    let url = broker.url();
    let server = start_with(
        Arc::new(OsEntropy),
        &[
            "--mqtt-url",
            &url,
            "--mqtt-username",
            "afterimage",
            "--mqtt-password",
            "deployment secret",
        ],
    )
    .await;
    server.sign_up().await;
    let shown = settings(&server).await;
    assert_eq!(shown["setByDeployment"], true);
    assert_eq!(shown["passwordSet"], true);
    assert!(!shown.to_string().contains("deployment secret"));
    let (status, body) = server
        .call(
            "PUT",
            SETTINGS,
            Some(json!({ "url": "mqtt://broker.example.test:1883" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}
