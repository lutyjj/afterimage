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

/// Every operation, from the `OpenAPI` document generated from the model, with
/// whether the document says it takes no credential (`security: []`).
fn operations() -> Vec<(String, String, bool)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../gen/management-api.openapi.json"
    );
    let document: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut operations = Vec::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            let open = operation.get("security") == Some(&json!([]));
            operations.push((method.to_uppercase(), path.clone(), open));
        }
    }
    operations
}

#[tokio::test]
async fn every_operation_but_signing_in_needs_a_session() {
    let server = start().await;
    let operations = operations();
    assert!(operations.len() > SIGNED_OUT.len());
    for (method, path, open) in operations {
        let key = format!("{method} {path}");
        // The model's `@auth([])` and the server's `NeedsSession` agree.
        assert_eq!(open, SIGNED_OUT.contains(&key.as_str()), "{key}");
        if open {
            continue;
        }
        let concrete = path
            .replace("{deviceId}", "1")
            .replace("{screenId}", "1")
            .replace("{tokenId}", "1");
        let response = server
            .http
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
                format!("{}{concrete}", server.url),
            )
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{key}");
        // restJson1's error form, which the generated client decodes, and the
        // bearer challenge RFC 6750 asks of a 401.
        assert_eq!(
            response.headers()["x-amzn-errortype"],
            "Unauthenticated",
            "{key}"
        );
        assert_eq!(response.headers()["www-authenticate"], "Bearer", "{key}");
        let body: Value = response.json().await.unwrap();
        assert!(body["message"].is_string(), "{key}: {body}");
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
    assert_eq!(
        server.display(&key, &READINGS).await["status"],
        0,
        "a claimed device shows a screen"
    );
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
    let rows = server
        .stored("SELECT expires_at IS NULL FROM registrations ORDER BY id")
        .await;
    let claimed: Vec<bool> = rows.iter().map(|row| row.get(0)).collect();
    assert_eq!(
        claimed,
        [false, true],
        "the old registration is gone; the leftover ends on its own"
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
            .tls_certs_only([])
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
        data: std::sync::Arc::new(tempfile::TempDir::new().unwrap()),
        http: reqwest::Client::builder()
            .tls_certs_only([])
            .build()
            .unwrap(),
        trmnl: server.trmnl.clone(),
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
    for (member, value) in [
        ("gap", "medium"),
        ("align", "start"),
        ("justify", "start"),
        ("padding", "none"),
        ("rounded", "none"),
    ] {
        column[member] = json!(value);
    }
    column["grow"] = json!(false);
    column["inverse"] = json!(false);
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
    // TRMNL lists openframe, but it takes WebP images, which Afterimage does not draw.
    let webp = json!({ "display": { "model": "openframe", "palette": "color-24bit" } });
    assert_eq!(
        server.call("PATCH", &path, Some(webp)).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, body) = server.call("GET", "/manage/display-models", None).await;
    assert!(
        body["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|model| model["name"] != "openframe"),
        "only displays Afterimage can draw are offered"
    );
}

#[tokio::test]
async fn display_models_carry_the_colors_their_palettes_show() {
    let server = start().await;
    server.sign_up().await;
    let (status, body) = server.call("GET", "/manage/display-models", None).await;
    assert_eq!(status, StatusCode::OK);
    let og_plus = body["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["name"] == "og_plus")
        .unwrap();
    let palette = |id: &str| {
        og_plus["palettes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|palette| palette["id"] == id)
            .unwrap()
            .clone()
    };
    // TRMNL lists four grays for gray-4; the renderer spreads them evenly.
    assert_eq!(
        palette("gray-4")["colors"],
        json!(["#000000", "#555555", "#aaaaaa", "#ffffff"])
    );
    assert_eq!(palette("bw")["colors"], json!(["#000000", "#ffffff"]));
}

#[tokio::test]
async fn the_server_names_its_time_zone_and_secrets_but_no_secret_values() {
    let server = common::start_with_secrets(afterimage_server::secrets::Secrets::from_vars([(
        "AFTERIMAGE_SECRET_HA_TOKEN".into(),
        "token-value".into(),
    )]))
    .await;
    server.sign_up().await;
    let response = server
        .http
        .get(format!("{}/manage/server", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let text = response.text().await.unwrap();
    assert!(!text.contains("token-value"));
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["timeZone"], "UTC");
    assert_eq!(body["secretNames"], json!(["HA_TOKEN"]));

    // The default display is the one a device that reported neither a known model
    // nor its panel size starts with.
    let (_, code) = server.register(&mac(1)).await;
    let (status, claimed) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{claimed}");
    assert_eq!(body["defaultDisplay"], claimed["device"]["display"]);
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
    for _ in 0..2 {
        // Repeated, in case the device never received an answer.
        let answer = server.display(&key, &READINGS).await;
        assert_eq!(
            (&answer["status"], &answer["reset_firmware"]),
            (&json!(500), &json!(true))
        );
        server.clock.advance(60 * 60);
    }
    server
        .clock
        .advance(afterimage_server::registrations::IDLE_SECONDS);
    server.register(&mac(2)).await;
    let macs: Vec<String> = server
        .stored("SELECT mac FROM registrations")
        .await
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(macs, [mac(2)], "the reset registration ends");
}

#[tokio::test]
async fn another_accounts_claim_leaves_a_waiting_device_alone() {
    let server = start().await;
    server.sign_up().await;
    // The real device waits to be claimed; someone presents its MAC and claims that.
    let (key, code) = server.register(&mac(1)).await;
    let (_, spoofed) = server.register(&mac(1)).await;
    let (status, _) = server
        .call("POST", "/manage/devices", Some(json!({ "code": spoofed })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(server.display(&key, &READINGS).await["status"], 202);
    let codes: Vec<String> = server
        .stored("SELECT friendly_id FROM registrations WHERE expires_at IS NOT NULL")
        .await
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(codes, [code], "the real device can still be claimed");
}

#[tokio::test]
async fn a_data_source_cannot_take_a_name_templates_read() {
    let server = start().await;
    server.sign_up().await;
    for name in ["now", "device", "secrets", "item", "hidden"] {
        let mut document = document();
        document["sources"] = json!({ name: { "url": "https://ha.example.test/" } });
        let (status, body) = server
            .call(
                "POST",
                "/manage/screens",
                Some(json!({ "name": "Clash", "document": document })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}: {body}");
    }
}

#[tokio::test]
async fn a_session_ends_after_thirty_days() {
    let server = start().await;
    server.sign_up().await;
    server
        .clock
        .advance(afterimage_server::sessions::SESSION_SECONDS - 1);
    assert_eq!(
        server.call("GET", "/manage/account", None).await.0,
        StatusCode::OK
    );
    server.clock.advance(1);
    assert_eq!(
        server.call("GET", "/manage/account", None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

/// A session that cannot be read is a server failure, in restJson1's error form,
/// and not a sign-out.
#[tokio::test]
async fn a_session_that_cannot_be_read_is_a_server_failure() {
    let server = start().await;
    server.sign_up().await;
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        server.data.path().join("afterimage.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("DROP TABLE sessions")
        .execute(&pool)
        .await
        .unwrap();
    let response = server
        .http
        .get(format!("{}/manage/account", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["x-amzn-errortype"], "InternalFailure");
    let body: Value = response.json().await.unwrap();
    assert!(body["message"].is_string(), "{body}");
}
