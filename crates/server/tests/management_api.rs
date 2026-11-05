//! The management API as the web UI uses it, over the assembled server.

mod common;

use common::{READINGS, mac, start};
use reqwest::StatusCode;
use serde_json::{Value, json};
use sqlx::Row;

/// The operations that start a session; every other one needs a session.
const SIGNED_OUT: [&str; 5] = [
    "GET /manage/auth",
    "POST /manage/auth/first-account",
    "POST /manage/auth/password",
    "GET /manage/auth/oidc",
    "GET /manage/auth/oidc/callback",
];

/// Every operation, from the `OpenAPI` document generated from the model.
fn operations() -> Vec<(String, String)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../gen/management-api.openapi.json"
    );
    let document: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut operations = Vec::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        for method in item.as_object().unwrap().keys() {
            operations.push((method.to_uppercase(), path.clone()));
        }
    }
    operations
}

#[tokio::test]
async fn every_operation_but_signing_in_needs_a_session() {
    let server = start().await;
    let operations = operations();
    assert!(operations.len() > SIGNED_OUT.len());
    for (method, path) in operations {
        let key = format!("{method} {path}");
        if SIGNED_OUT.contains(&key.as_str()) {
            continue;
        }
        let concrete = path.replace("{deviceId}", "1").replace("{screenId}", "1");
        let (status, body) = server.call(&method, &concrete, Some(json!({}))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{key}: {body}");
    }
}

#[tokio::test]
async fn the_first_account_signs_up_once_and_signs_in_with_its_password() {
    let server = start().await;
    let (_, options) = server.call("GET", "/manage/auth", None).await;
    assert_eq!(options, json!({ "firstAccount": true, "password": true }));
    let account = server.sign_up().await;
    assert_eq!(account["username"], "ada");
    let (status, _) = server
        .call(
            "POST",
            "/manage/auth/first-account",
            Some(json!({ "name": "Eve", "username": "eve", "password": "another password" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        server.call("GET", "/manage/auth", None).await.1["firstAccount"],
        false
    );

    let (status, body) = server.call("GET", "/manage/account", None).await;
    assert_eq!((status, &body["account"]), (StatusCode::OK, &account));
    assert_eq!(
        server.call("POST", "/manage/auth/sign-out", None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        server.call("GET", "/manage/account", None).await.0,
        StatusCode::UNAUTHORIZED
    );

    let wrong = json!({ "username": "ada", "password": "correct horse batter" });
    let (status, _) = server
        .call("POST", "/manage/auth/password", Some(wrong))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let unknown = json!({ "username": "nobody", "password": "correct horse battery" });
    assert_eq!(
        server
            .call("POST", "/manage/auth/password", Some(unknown))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let right = json!({ "username": "ada", "password": "correct horse battery" });
    let (status, body) = server
        .call("POST", "/manage/auth/password", Some(right))
        .await;
    assert_eq!((status, &body["account"]), (StatusCode::OK, &account));
    assert_eq!(
        server.call("GET", "/manage/account", None).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn stores_only_a_hash_of_the_password_and_the_session() {
    let server = start().await;
    server.sign_up().await;
    let rows = server.stored("SELECT password_hash FROM accounts").await;
    let hash: String = rows[0].get(0);
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    for path in server.database_files() {
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes.windows(9).any(|window| window == b"correct h"),
            "{} holds the password",
            path.display()
        );
    }
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

#[tokio::test]
async fn claiming_a_code_makes_the_registration_a_device_with_a_default_display() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    assert_eq!(device["mac"], mac(1));
    assert_eq!(device["firmwareModel"], "og");
    assert_eq!(device["refreshMinutes"], 15);
    assert_eq!(device["playlist"], json!([]));
    assert_eq!(device["readings"]["batteryVoltage"], 4.12);
    // The test device reports an 800x480 panel and a model name the catalogue lacks.
    let (_, models) = server.call("GET", "/manage/display-models", None).await;
    let chosen = models["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["name"] == device["display"]["model"])
        .unwrap();
    assert_eq!(
        (chosen["width"].as_i64(), chosen["height"].as_i64()),
        (Some(800), Some(480))
    );
    assert!(
        chosen["palettes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|palette| palette["id"] == device["display"]["palette"])
    );
    let (_, devices) = server.call("GET", "/manage/devices", None).await;
    assert_eq!(devices["devices"], json!([device]));
    let rows = server.stored("SELECT expires_at FROM registrations").await;
    assert_eq!(
        rows[0].get::<Option<i64>, _>(0),
        None,
        "a claimed registration does not end"
    );
    // Past the idle end of an unclaimed registration, within the session.
    server
        .clock
        .advance(afterimage_server::registrations::IDLE_SECONDS + 24 * 60 * 60);
    assert_eq!(server.display(&key, &READINGS).await["status"], 202);
    assert_eq!(
        server.call("GET", "/manage/devices", None).await.1["devices"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_code_nobody_shows_claims_nothing() {
    let server = start().await;
    server.sign_up().await;
    let (status, _) = server
        .call("POST", "/manage/devices", Some(json!({ "code": "ZZZZZZ" })))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = server
        .call(
            "POST",
            "/manage/devices",
            Some(json!({ "code": "not a code" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn claiming_a_device_set_up_again_keeps_it_and_retires_its_old_key() {
    let server = start().await;
    server.sign_up().await;
    let (device, old_key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let screen = create_screen(&server, "Clock").await;
    let playlist = json!({ "playlist": [{ "screenId": screen }] });
    let (status, _) = server
        .call(
            "PUT",
            &format!("/manage/devices/{id}/playlist"),
            Some(playlist),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "name": "Kitchen" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // A leftover setup of the same MAC, then the one the device shows.
    server.register(&mac(1)).await;
    let (new_key, code) = server.register(&mac(1)).await;
    let (status, body) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["replaced"], true);
    assert_eq!(body["device"]["id"], id);
    assert_eq!(body["device"]["name"], "Kitchen");
    assert_eq!(body["device"]["playlist"], json!([{ "screenId": screen }]));
    let (_, devices) = server.call("GET", "/manage/devices", None).await;
    assert_eq!(devices["devices"].as_array().unwrap().len(), 1);
    let rows = server.stored("SELECT expires_at FROM registrations").await;
    assert_eq!(
        rows.len(),
        1,
        "the old and the leftover registrations are gone"
    );
    assert_eq!(
        server
            .log(&old_key, json!({ "logs": [] }).to_string())
            .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        server
            .log(&new_key, json!({ "logs": [] }).to_string())
            .await,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn another_account_cannot_reach_a_device_or_screen() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let device = device["id"].as_i64().unwrap();
    let screen = create_screen(&server, "Private").await;

    // A second account, which only the OIDC provider creates, written directly.
    let hash = password_auth::generate_hash("second password here");
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("INSERT INTO accounts (name, username, password_hash, created_at) VALUES ('Eve', 'eve', ?, 0)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();
    let eve = common::Server {
        http: reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap(),
        ..clone_server(&server)
    };
    let (status, _) = eve
        .call(
            "POST",
            "/manage/auth/password",
            Some(json!({ "username": "eve", "password": "second password here" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    for (method, path, body) in [
        ("GET", format!("/manage/devices/{device}"), None),
        (
            "PATCH",
            format!("/manage/devices/{device}"),
            Some(json!({ "name": "Mine" })),
        ),
        (
            "PUT",
            format!("/manage/devices/{device}/playlist"),
            Some(json!({ "playlist": [] })),
        ),
        ("DELETE", format!("/manage/devices/{device}"), None),
        ("GET", format!("/manage/screens/{screen}"), None),
        (
            "PUT",
            format!("/manage/screens/{screen}"),
            Some(json!({ "name": "Mine", "document": document() })),
        ),
        ("DELETE", format!("/manage/screens/{screen}"), None),
    ] {
        assert_eq!(
            eve.call(method, &path, body).await.0,
            StatusCode::NOT_FOUND,
            "{method} {path}"
        );
    }
    assert_eq!(
        eve.call("GET", "/manage/devices", None).await.1["devices"],
        json!([])
    );
    assert_eq!(
        eve.call("GET", "/manage/screens", None).await.1["screens"],
        json!([])
    );

    // Eve's playlist cannot name Ada's screen either.
    let (_, code) = eve.register(&mac(2)).await;
    let (_, claimed) = eve
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    let eves = claimed["device"]["id"].as_i64().unwrap();
    let (status, _) = eve
        .call(
            "PUT",
            &format!("/manage/devices/{eves}/playlist"),
            Some(json!({ "playlist": [{ "screenId": screen }] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        server
            .call("GET", &format!("/manage/devices/{device}"), None)
            .await
            .0,
        StatusCode::OK
    );
}

/// The same server, reached through another client.
fn clone_server(server: &common::Server) -> common::Server {
    common::Server {
        url: server.url.clone(),
        clock: server.clock.clone(),
        data: tempfile::TempDir::new().unwrap(),
        http: reqwest::Client::new(),
    }
}

fn document() -> Value {
    json!({
        "sources": { "weather": { "url": "https://ha.example.test/api/states/weather.home" } },
        "titleBar": { "title": "Weather" },
        "content": {
            "column": {
                "children": [
                    { "value": { "text": "{{ weather.attributes.temperature }}°", "size": "xlarge" } },
                    { "label": { "text": "{{ weather.state }}" } }
                ]
            }
        }
    })
}

async fn create_screen(server: &common::Server, name: &str) -> i64 {
    let (status, body) = server
        .call(
            "POST",
            "/manage/screens",
            Some(json!({ "name": name, "document": document() })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["screen"]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn a_screen_document_comes_back_as_written_with_its_defaults() {
    let server = start().await;
    server.sign_up().await;
    let id = create_screen(&server, "Weather").await;
    let (status, body) = server
        .call("GET", &format!("/manage/screens/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let mut expected = document();
    expected["sources"]["weather"]["method"] = json!("GET");
    let column = &mut expected["content"]["column"];
    for (member, value) in [("gap", "medium"), ("align", "start"), ("justify", "start")] {
        column[member] = json!(value);
    }
    column["grow"] = json!(false);
    for child in column["children"].as_array_mut().unwrap() {
        if let Some(value) = child.get_mut("value") {
            value["fit"] = json!(false);
            value["tabular"] = json!(false);
        }
        if let Some(label) = child.get_mut("label") {
            label["size"] = json!("base");
            label["style"] = json!("plain");
        }
    }
    assert_eq!(body["screen"]["document"], expected);
    assert_eq!(body["screen"]["name"], "Weather");

    let (_, list) = server.call("GET", "/manage/screens", None).await;
    assert_eq!(list["screens"][0]["id"], id);
    let invalid =
        json!({ "name": "Broken", "document": { "content": { "column": { "children": [] } } } });
    assert_eq!(
        server
            .call("POST", "/manage/screens", Some(invalid))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn deleting_a_screen_takes_it_out_of_every_playlist() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let kept = create_screen(&server, "Kept").await;
    let dropped = create_screen(&server, "Dropped").await;
    let playlist = json!({ "playlist": [
        { "screenId": dropped, "days": ["monday", "friday"], "from": "07:00", "until": "09:30" },
        { "screenId": kept }
    ] });
    let (status, body) = server
        .call(
            "PUT",
            &format!("/manage/devices/{id}/playlist"),
            Some(playlist.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["device"]["playlist"], playlist["playlist"]);
    assert_eq!(
        server
            .call("DELETE", &format!("/manage/screens/{dropped}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(body["device"]["playlist"], json!([{ "screenId": kept }]));
}

#[tokio::test]
async fn a_display_must_be_one_the_catalogue_lists() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let path = format!("/manage/devices/{id}");
    let wrong = json!({ "display": { "model": "og_plus", "palette": "color-6a" } });
    assert_eq!(
        server.call("PATCH", &path, Some(wrong)).await.0,
        StatusCode::BAD_REQUEST
    );
    let right = json!({ "display": { "model": "og_png", "palette": "bw" }, "refreshMinutes": 30 });
    let (status, body) = server.call("PATCH", &path, Some(right)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["device"]["display"],
        json!({ "model": "og_png", "palette": "bw" })
    );
    assert_eq!(body["device"]["refreshMinutes"], 30);
}

#[tokio::test]
async fn a_removed_device_shows_its_code_again_and_can_be_claimed_again() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    assert_eq!(
        server
            .call("DELETE", &format!("/manage/devices/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.display(&key, &READINGS).await["status"], 202);
    let code: String = server.stored("SELECT friendly_id FROM registrations").await[0].get(0);
    let (status, _) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

/// Removing with a reset makes the next check-in erase the device (`status` 500
/// and `reset_firmware`, `bl.cpp`); the registration goes with it.
#[tokio::test]
async fn a_device_removed_with_a_reset_is_erased_at_its_next_check_in() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let (status, _) = server
        .call("DELETE", &format!("/manage/devices/{id}?reset=true"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(
        (&answer["status"], &answer["reset_firmware"]),
        (&json!(500), &json!(true))
    );
    assert!(
        server
            .stored("SELECT id FROM registrations")
            .await
            .is_empty()
    );
    assert_eq!(
        server.display(&key, &READINGS).await["status"],
        202,
        "erased once"
    );
}
