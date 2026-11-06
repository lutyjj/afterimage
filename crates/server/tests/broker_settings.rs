//! The MQTT broker set in the web UI: only the administrator, the first account,
//! reads and changes it; it takes effect at once, moving or clearing what the
//! server published; the password is written and never read back; and where the
//! deployment sets the broker, it cannot change here.

mod common;

use std::sync::Arc;

use common::{Broker, Observer, Server, mac, start, start_with};
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
        url: server.url.clone(),
        clock: Arc::clone(&server.clock),
        data: Arc::clone(&server.data),
        http: reqwest::Client::builder()
            .tls_certs_only([])
            .cookie_store(true)
            .build()
            .unwrap(),
        trmnl: server.trmnl.clone(),
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
