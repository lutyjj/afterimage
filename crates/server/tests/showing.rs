//! What claimed devices show: screens drawn from data sources, served to the
//! firmware as images it downloads with its key, and previews for the editor.

mod common;

use std::sync::{Arc, Mutex};

use afterimage_server::clock::SystemClock;
use afterimage_server::devices::{Devices, Showing, Step};
use afterimage_server::playlists::Playlists;
use afterimage_server::secrets::Secrets;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use common::{FIRMWARE, READINGS, mac, start, start_on_data, start_with_secrets};
use reqwest::StatusCode;
use serde_json::{Value, json};

/// A JSON endpoint whose answer the test changes, which records the
/// `Authorization` it was sent.
#[derive(Clone, Default)]
struct Source {
    answer: Arc<Mutex<Option<Value>>>,
    authorization: Arc<Mutex<Option<String>>>,
}

impl Source {
    async fn serve(&self) -> String {
        async fn answer(
            State(source): State<Source>,
            headers: HeaderMap,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;
            *source.authorization.lock().unwrap() = headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            match source.answer.lock().unwrap().clone() {
                Some(value) => Json(value).into_response(),
                None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            }
        }
        let app = axum::Router::new()
            .route("/state", get(answer))
            .with_state(self.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/state", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await });
        url
    }

    fn set(&self, answer: Option<Value>) {
        *self.answer.lock().unwrap() = answer;
    }
}

fn weather(url: &str) -> Value {
    json!({
        "sources": { "weather": { "url": url, "headers": { "Authorization": "Bearer {{ secrets.HA_TOKEN }}" } } },
        "titleBar": { "title": "Weather", "instance": "{{ device.name }}" },
        "content": { "column": { "children": [
            { "value": { "text": "{{ weather.temperature }}°", "size": "xlarge" } },
            { "label": { "text": "secret: [{{ secrets.HA_TOKEN }}]" } },
            { "lineChart": { "series": [{ "values": "{{ weather.hourly }}" }] } }
        ] } }
    })
}

async fn screen(server: &common::Server, name: &str, document: Value) -> i64 {
    let (status, body) = server
        .call(
            "POST",
            "/manage/screens",
            Some(json!({ "name": name, "document": document })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["screen"]["id"].as_i64().unwrap()
}

async fn playlist(server: &common::Server, name: &str, screens: &[i64]) -> i64 {
    let (status, body) = server
        .call(
            "POST",
            "/manage/playlists",
            Some(json!({ "name": name, "screens": screens })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["playlist"]["id"].as_i64().unwrap()
}

async fn set_schedule(
    server: &common::Server,
    device: i64,
    schedule: Value,
) -> (StatusCode, Value) {
    server
        .call(
            "PUT",
            &format!("/manage/devices/{device}/schedule"),
            Some(json!({ "schedule": schedule })),
        )
        .await
}

/// Gives the device a playlist of these screens to show usually.
async fn play(server: &common::Server, device: i64, screens: &[i64]) -> i64 {
    let id = playlist(server, "Usual", screens).await;
    let (status, body) =
        set_schedule(server, device, json!({ "playlistId": id, "slots": [] })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    id
}

/// Replaces the playlist's screens, keeping its name.
async fn refill(server: &common::Server, playlist: i64, screens: &[i64]) {
    let (status, body) = server
        .call(
            "PUT",
            &format!("/manage/playlists/{playlist}"),
            Some(json!({ "name": "Usual", "screens": screens })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// The image a check-in names, downloaded as firmware does, with the key.
async fn download(server: &common::Server, answer: &Value, key: &str) -> reqwest::Response {
    server
        .http
        .get(answer["image_url"].as_str().unwrap())
        .header("ID", "41:B4:10:39:A1:01")
        .header("Access-Token", key)
        .send()
        .await
        .unwrap()
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    let reader = png::Decoder::new(std::io::Cursor::new(bytes.to_vec()))
        .read_info()
        .unwrap();
    (reader.info().width, reader.info().height)
}

#[tokio::test]
async fn a_claimed_device_shows_its_screen_drawn_from_its_data() {
    let source = Source::default();
    source.set(Some(json!({ "temperature": 7.4, "hourly": [5, 7, 6, 9] })));
    let url = source.serve().await;
    let server = start_with_secrets(Secrets::from_vars([(
        "AFTERIMAGE_SECRET_HA_TOKEN".into(),
        "s3cret".into(),
    )]))
    .await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let weather = screen(&server, "Weather", weather(&url)).await;
    play(&server, id, &[weather]).await;

    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["status"], 0, "{answer}");
    assert_eq!(answer["refresh_rate"], 15 * 60);
    let image_url = answer["image_url"].as_str().unwrap();
    assert!(
        image_url.starts_with(&format!("{}/api/images/", server.url)),
        "{image_url}"
    );
    let filename = answer["filename"].as_str().unwrap();
    assert!(
        filename.len() <= 31,
        "the firmware keeps 31 characters: {filename}"
    );
    assert_eq!(
        source.authorization.lock().unwrap().as_deref(),
        Some("Bearer s3cret")
    );

    let response = download(&server, &answer, &key).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    let bytes = response.bytes().await.unwrap();
    let (width, height) = png_size(&bytes);
    let catalog = afterimage_trmnl::catalog::Catalog::get();
    let model = catalog
        .model(device["display"]["model"].as_str().unwrap())
        .unwrap();
    assert_eq!((width, height), (model.width, model.height));

    // The editor sees the same image as the device.
    let shown = server
        .http
        .get(format!("{}/manage/devices/{id}/image", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(shown.status(), StatusCode::OK);
    assert_eq!(shown.bytes().await.unwrap(), bytes);
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(body["device"]["showing"]["screenId"], weather);

    let current = server.current(&key).await;
    assert_eq!(
        (&current["status"], &current["filename"]),
        (&json!(0), &json!(filename))
    );
}

#[tokio::test]
async fn only_the_device_the_image_was_drawn_for_downloads_it() {
    let server = start().await;
    server.sign_up().await;
    let (_, key) = server.claim(&mac(1)).await;
    let (_, other) = server.claim(&mac(2)).await;
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(
        download(&server, &answer, &key).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        download(&server, &answer, &other).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        download(&server, &answer, common::UNKNOWN_KEY)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_unchanged_screen_keeps_its_name_and_an_outage_keeps_the_last_image() {
    let source = Source::default();
    source.set(Some(json!({ "temperature": 7.4, "hourly": [1, 2] })));
    let url = source.serve().await;
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let weather = screen(&server, "Weather", weather(&url)).await;
    play(&server, id, &[weather]).await;

    let first = server.display(&key, &READINGS).await["filename"].clone();
    assert_eq!(
        server.display(&key, &READINGS).await["filename"],
        first,
        "the firmware's cache holds"
    );
    source.set(Some(json!({ "temperature": 9.1, "hourly": [1, 2] })));
    let changed = server.display(&key, &READINGS).await;
    assert_ne!(changed["filename"], first);
    source.set(None);
    let during = server.display(&key, &READINGS).await;
    assert_eq!(
        during["filename"], changed["filename"],
        "the display keeps its last image"
    );
    assert_eq!(
        download(&server, &during, &key).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_screen_never_shows_a_secret() {
    let source = Source::default();
    source.set(Some(json!({ "temperature": 7.4, "hourly": [1, 2] })));
    let url = source.serve().await;
    let server = start_with_secrets(Secrets::from_vars([(
        "AFTERIMAGE_SECRET_HA_TOKEN".into(),
        "s3cret".into(),
    )]))
    .await;
    server.sign_up().await;
    let (_, body) = server
        .call(
            "POST",
            "/manage/preview",
            Some(json!({ "document": weather(&url), "display": { "model": "og_plus", "palette": "gray-4" } })),
        )
        .await;
    assert_eq!(body["problems"], json!([]), "{body}");
    assert_eq!(
        source.authorization.lock().unwrap().as_deref(),
        Some("Bearer s3cret")
    );
    // View templates do not have `secrets`: the label above showed nothing, and a
    // template that needs a secret's value fails because there is none.
    let document = json!({ "content": { "value": { "text": "{{ secrets.HA_TOKEN | length }}" } } });
    let (_, body) = server
        .call(
            "POST",
            "/manage/preview",
            Some(json!({ "document": document, "display": { "model": "og_plus", "palette": "gray-4" } })),
        )
        .await;
    let problem = &body["problems"][0];
    assert_eq!(problem["path"], "/content/value/text", "{body}");
    assert!(
        problem["message"].as_str().unwrap().contains("undefined"),
        "{body}"
    );
    assert!(body["image"].is_string());
}

#[tokio::test]
async fn a_slot_takes_over_from_the_usual_playlist_and_wakes_the_device_for_it() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let usual_screen = screen(&server, "Usual", text("Usual")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let third = screen(&server, "Third", text("Third")).await;
    let usual = playlist(&server, "Usual", &[usual_screen]).await;
    let mornings = playlist(&server, "Mornings", &[second, third]).await;
    // The harness clock starts at 08:00 UTC; the slot starts at 08:05.
    let (status, body) = set_schedule(
        &server,
        id,
        json!({ "playlistId": usual, "slots": [{ "playlistId": mornings, "from": "08:05", "until": "20:00" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let showing = |device: Value| {
        (
            device["showing"]["screenId"].clone(),
            device["showing"]["playlistId"].clone(),
        )
    };

    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["refresh_rate"], 5 * 60, "wakes when the slot starts");
    assert_eq!(
        showing(device(&server, id).await),
        (json!(usual_screen), json!(usual))
    );

    server.clock.advance(5 * 60);
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(device(&server, id).await),
        (json!(second), json!(mornings)),
        "the slot's playlist starts at its first screen"
    );
    server.display(&key, &READINGS).await;
    assert_eq!(showing(device(&server, id).await).0, third);
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(device(&server, id).await).0,
        second,
        "and starts over"
    );
}

#[tokio::test]
async fn a_schedule_whose_slots_are_current_at_once_is_refused() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, _) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let one = playlist(&server, "One", &[]).await;
    let two = playlist(&server, "Two", &[]).await;
    let (status, body) = set_schedule(
        &server,
        id,
        json!({ "slots": [
            { "playlistId": one, "days": ["monday"], "from": "07:00", "until": "09:00" },
            { "playlistId": two, "from": "08:30", "until": "12:00" }
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Slots 1 and 2 are both current on Monday at 08:30"),
        "{body}"
    );
    assert_eq!(
        device(&server, id).await["schedule"],
        json!({ "slots": [] }),
        "nothing changed"
    );

    let meeting = json!({ "playlistId": one, "slots": [
        { "playlistId": one, "days": ["monday"], "from": "07:00", "until": "09:00" },
        { "playlistId": two, "days": ["monday"], "from": "09:00", "until": "07:00" }
    ] });
    let (status, body) = set_schedule(&server, id, meeting.clone()).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "slots that meet do not overlap: {body}"
    );
    assert_eq!(body["device"]["schedule"], meeting);
}

#[tokio::test]
async fn devices_share_a_playlist_and_each_goes_through_it_on_its_own() {
    let server = start().await;
    server.sign_up().await;
    let (one, one_key) = server.claim(&mac(1)).await;
    let (two, two_key) = server.claim(&mac(2)).await;
    let (one, two) = (one["id"].as_i64().unwrap(), two["id"].as_i64().unwrap());
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let shared = play(&server, one, &[first, second]).await;
    let (status, body) =
        set_schedule(&server, two, json!({ "playlistId": shared, "slots": [] })).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    server.display(&one_key, &READINGS).await;
    server.display(&one_key, &READINGS).await;
    server.display(&two_key, &READINGS).await;
    assert_eq!(device(&server, one).await["showing"]["screenId"], second);
    assert_eq!(
        device(&server, two).await["showing"]["screenId"],
        first,
        "the other device starts at the first screen"
    );

    // Deleting the playlist leaves both with nothing scheduled.
    let (status, _) = server
        .call("DELETE", &format!("/manage/playlists/{shared}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        device(&server, one).await["schedule"],
        json!({ "slots": [] })
    );
    server.display(&two_key, &READINGS).await;
    let shown = device(&server, two).await;
    assert!(shown["showing"].get("screenId").is_none(), "{shown}");
    assert!(shown["showing"].get("playlistId").is_none(), "{shown}");
}

#[tokio::test]
async fn a_device_with_nothing_current_shows_the_servers_screen() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["status"], 0);
    assert_eq!(
        download(&server, &answer, &key).await.status(),
        StatusCode::OK
    );
    let id = device["id"].as_i64().unwrap();
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert!(
        body["device"]["showing"].get("screenId").is_none(),
        "{body}"
    );
}

#[tokio::test]
async fn a_preview_reports_each_source_and_each_problem_by_path() {
    let source = Source::default();
    source.set(Some(json!({ "temperature": 7.4 })));
    let url = source.serve().await;
    let server = start().await;
    server.sign_up().await;
    let document = json!({
        "sources": {
            "weather": { "url": url },
            "down": { "url": "http://127.0.0.1:9/nothing" }
        },
        "content": { "column": { "children": [
            { "value": { "text": "{{ weather.temperature }}°" } },
            { "label": { "text": "{{ 1 + }}" } }
        ] } }
    });
    let (status, body) = server
        .call(
            "POST",
            "/manage/preview",
            Some(
                json!({ "document": document, "display": { "model": "og_png", "palette": "bw" } }),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sources"]["weather"]["data"],
        json!({ "temperature": 7.4 })
    );
    assert!(body["sources"]["down"]["error"].is_string(), "{body}");
    let paths: Vec<&str> = body["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|problem| problem["path"].as_str())
        .collect();
    assert!(paths.contains(&"/sources/down"), "{paths:?}");
    assert!(
        paths.contains(&"/content/column/children/1/label/text"),
        "{paths:?}"
    );
    assert!(body["image"].is_string(), "the screen is still drawn");

    let wrong =
        json!({ "document": document, "display": { "model": "og_png", "palette": "gray-4" } });
    assert_eq!(
        server.call("POST", "/manage/preview", Some(wrong)).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn a_secret_stays_out_of_failures_and_echoed_answers() {
    let echo = Source::default();
    // An answer that repeats what it was sent, as some APIs do.
    echo.set(Some(json!({ "sent": "Bearer s3cret-value" })));
    let url = echo.serve().await;
    let server = start_with_secrets(Secrets::from_vars([(
        "AFTERIMAGE_SECRET_HA_TOKEN".into(),
        "s3cret-value".into(),
    )]))
    .await;
    server.sign_up().await;
    let document = json!({
        "sources": {
            // Nothing listens on port 9, so the request fails with the secret in its URL.
            "down": { "url": "http://127.0.0.1:9/state?token={{ secrets.HA_TOKEN }}" },
            "echo": { "url": url, "headers": { "Authorization": "Bearer {{ secrets.HA_TOKEN }}" } },
            "misnamed": { "url": "{{ secrets.HA_TOKEN }}" }
        },
        "content": { "label": { "text": "{{ echo.sent }}" } }
    });
    let response = server
        .http
        .post(format!("{}/manage/preview", server.url))
        .json(&json!({ "document": document, "display": { "model": "og_plus", "palette": "gray-4" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let text = response.text().await.unwrap();
    assert!(!text.contains("s3cret-value"), "{text}");
    let body: Value = serde_json::from_str(&text).unwrap();
    assert!(body["sources"]["down"]["error"].is_string(), "{body}");
    assert_eq!(
        body["sources"]["echo"]["data"]["sent"],
        "Bearer [secret HA_TOKEN]"
    );
    assert_eq!(
        echo.authorization.lock().unwrap().as_deref(),
        Some("Bearer s3cret-value"),
        "the request itself carries the secret"
    );
}

#[tokio::test]
async fn a_screen_that_fails_does_not_hold_up_the_rest_of_the_playlist() {
    let down = Source::default();
    let url = down.serve().await;
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let broken = screen(
        &server,
        "Broken",
        json!({ "sources": { "down": { "url": url } }, "content": { "title": { "text": "{{ down.value }}" } } }),
    )
    .await;
    let third = screen(&server, "Third", text("Third")).await;
    play(&server, id, &[first, broken, third]).await;
    let showing = |body: Value| body["device"]["showing"]["screenId"].clone();
    let device_path = format!("/manage/devices/{id}");

    let one = server.display(&key, &READINGS).await;
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        first
    );
    let two = server.display(&key, &READINGS).await;
    assert_eq!(
        two["filename"], one["filename"],
        "the device keeps its image"
    );
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        third,
        "the screen after the broken one shows next"
    );
}

#[tokio::test]
async fn a_changed_playlist_starts_again_at_its_first_screen() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let other = screen(&server, "Other", text("Other")).await;
    let usual = play(&server, id, &[first, second]).await;
    let shown = server.display(&key, &READINGS).await;
    let device_path = format!("/manage/devices/{id}");
    let showing = |body: Value| body["device"]["showing"]["screenId"].clone();
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        first
    );

    refill(&server, usual, &[other, first]).await;
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        other
    );

    // Deleting a screen of the playlist starts it over too. With [other, first,
    // second, last], the device shows `first`; deleting `other` leaves [first,
    // second, last], and the next check-in starts at `first` rather than taking
    // the old position and skipping `second` for `last`.
    let last = screen(&server, "Last", text("Last")).await;
    refill(&server, usual, &[other, first, second, last]).await;
    server.display(&key, &READINGS).await;
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        first
    );
    assert_eq!(
        server
            .call("DELETE", &format!("/manage/screens/{other}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    server.display(&key, &READINGS).await;
    assert_eq!(
        showing(server.call("GET", &device_path, None).await.1),
        first
    );

    // The image a check-in named stays downloadable after the next one changes it.
    assert_eq!(
        download(&server, &shown, &key).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_new_name_alone_leaves_a_playlist_where_its_devices_stand() {
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let usual = play(&server, id, &[first, second]).await;
    server.display(&key, &READINGS).await;

    let (status, body) = server
        .call(
            "PUT",
            &format!("/manage/playlists/{usual}"),
            Some(json!({ "name": "Renamed", "screens": [first, second] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    server.display(&key, &READINGS).await;
    assert_eq!(
        self::device(&server, id).await["showing"]["screenId"],
        second,
        "it goes on to the second screen"
    );
}

/// The stores a check-in uses, on the server's own database, so a test can
/// stand between a check-in's reads and its record.
async fn stores(server: &common::Server) -> (Devices, Playlists) {
    let pool = afterimage_server::database::open(&server.data.path().join("afterimage.db"))
        .await
        .unwrap();
    (
        Devices::new(pool.clone(), Arc::new(SystemClock)),
        Playlists::new(pool),
    )
}

#[tokio::test]
async fn a_check_in_does_not_undo_a_playlist_change_made_while_it_drew() {
    let server = start().await;
    let account = server.sign_up().await["id"].as_i64().unwrap();
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let third = screen(&server, "Third", text("Third")).await;
    let usual = play(&server, id, &[first, second, third]).await;
    let (devices, playlists) = stores(&server).await;
    let drawn = |screen_id| Showing {
        image: "drawn".to_owned(),
        screen_id: Some(screen_id),
        playlist_id: Some(usual),
        rendered_at: 0,
        asleep: false,
    };

    // Before its first check-in and after one, a check-in reads the
    // playlist, the playlist's screens change before it records its step, and
    // the change's fresh start stands.
    for (index, shown, changed) in [
        (0, first, [third, first, second]),
        (1, second, [second, third, first]),
    ] {
        let read = playlists.get(account, usual).await.unwrap().unwrap();
        refill(&server, usual, &changed).await;
        let step = Step {
            playlist_id: usual,
            screens: &read.screens,
            index,
        };
        devices
            .show(id, Some(step), &drawn(shown), 0)
            .await
            .unwrap();
        server.display(&key, &READINGS).await;
        assert_eq!(
            self::device(&server, id).await["showing"]["screenId"],
            changed[0],
            "the changed playlist starts at its first screen"
        );
    }
}

#[tokio::test]
async fn a_check_in_records_its_image_when_what_it_drew_from_is_deleted_meanwhile() {
    let server = start().await;
    let account = server.sign_up().await["id"].as_i64().unwrap();
    let (device, _) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let only = screen(
        &server,
        "Only",
        json!({ "content": { "title": { "text": "Only" } } }),
    )
    .await;
    let usual = play(&server, id, &[only]).await;
    let (devices, playlists) = stores(&server).await;
    let read = playlists.get(account, usual).await.unwrap().unwrap();
    for path in [
        format!("/manage/playlists/{usual}"),
        format!("/manage/screens/{only}"),
    ] {
        assert_eq!(
            server.call("DELETE", &path, None).await.0,
            StatusCode::NO_CONTENT
        );
    }
    let step = Step {
        playlist_id: usual,
        screens: &read.screens,
        index: 0,
    };
    let showing = Showing {
        image: "drawn".to_owned(),
        screen_id: Some(only),
        playlist_id: Some(usual),
        rendered_at: 0,
        asleep: false,
    };
    devices.show(id, Some(step), &showing, 0).await.unwrap();
    let shown = self::device(&server, id).await["showing"].clone();
    assert!(shown.get("screenId").is_none(), "{shown}");
    assert!(shown.get("playlistId").is_none(), "{shown}");
}

#[tokio::test]
async fn a_device_s_list_of_screens_becomes_the_playlist_it_usually_shows() {
    use afterimage_server::keys::token_hash;

    // A database at the schema deployments ran before playlists stood on their
    // own: a device whose list held three screens, one of them on Mondays only,
    // at positions with a gap, and which showed the second last.
    let data = tempfile::TempDir::new().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data.path().join("afterimage.db"))
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    let mut earlier = sqlx::migrate!("./migrations");
    earlier.migrations = earlier
        .migrations
        .iter()
        .filter(|migration| migration.version <= 7)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    earlier.run(&pool).await.unwrap();
    let hash = password_auth::generate_hash("correct horse battery");
    sqlx::query(
        "INSERT INTO accounts (id, name, username, password_hash, created_at) VALUES (1, 'Ada', 'ada', ?, 0)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();
    for (id, name) in [(10, "First"), (11, "Second"), (12, "Third")] {
        let document = json!({ "content": { "title": { "text": name, "size": "base" } } });
        sqlx::query(
            "INSERT INTO screens (id, account_id, name, document, updated_at) VALUES (?, 1, ?, ?, 0)",
        )
        .bind(id)
        .bind(name)
        .bind(document.to_string())
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO registrations (id, mac, friendly_id, key_hash, model, created_at, width, height)
         VALUES (3, '41:B4:10:39:A1:01', 'ABCDEF', ?, 'og', 0, 800, 480)",
    )
    .bind(token_hash("a-migrated-key"))
    .execute(&pool)
    .await
    .unwrap();
    for row in [
        "INSERT INTO devices (id, account_id, registration_id, name, display_model, palette,
                              refresh_minutes, created_at, playlist_position)
         VALUES (5, 1, 3, 'Kitchen', 'og_png', 'bw', 15, 0, 1)",
        "INSERT INTO playlist_entries (device_id, position, screen_id, days)
         VALUES (5, 0, 10, NULL), (5, 2, 11, 1), (5, 3, 12, NULL)",
    ] {
        sqlx::query(row).execute(&pool).await.unwrap();
    }
    pool.close().await;

    let server = start_on_data(data).await;
    let (status, body) = server
        .call(
            "POST",
            "/manage/auth/password",
            Some(json!({ "username": "ada", "password": "correct horse battery" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        server.call("GET", "/manage/playlists", None).await.1["playlists"],
        json!([{ "id": 5, "name": "Kitchen", "screens": [10, 11, 12] }]),
        "the screens in their order, without their days"
    );
    assert_eq!(
        device(&server, 5).await["schedule"],
        json!({ "playlistId": 5, "slots": [] })
    );
    server.display("a-migrated-key", &READINGS).await;
    let shown = device(&server, 5).await["showing"].clone();
    assert_eq!(
        (shown["screenId"].clone(), shown["playlistId"].clone()),
        (json!(12), json!(5)),
        "it goes on after the screen it showed last"
    );
}

#[tokio::test]
async fn a_device_whose_first_screen_fails_shows_why_and_a_lost_image_is_drawn_again() {
    let down = Source::default();
    let url = down.serve().await;
    let server = start().await;
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    // The first check-in after the claim draws the server's own screen.
    server.display(&key, &READINGS).await;
    let broken = screen(
        &server,
        "Broken",
        json!({ "sources": { "down": { "url": url } }, "content": { "title": { "text": "{{ down.value }}" } } }),
    )
    .await;
    play(&server, id, &[broken]).await;
    // Its file goes missing.
    std::fs::remove_dir_all(server.data.path().join("images").join(id.to_string())).unwrap();

    // The failing screen cannot keep an image whose file is gone: the device gets
    // the server's screen saying what went wrong, and can download it.
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["status"], 0, "{answer}");
    assert_eq!(
        download(&server, &answer, &key).await.status(),
        StatusCode::OK
    );
    let (_, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert!(
        body["device"]["showing"].get("screenId").is_none(),
        "{body}"
    );
}

#[tokio::test]
async fn a_device_reporting_a_model_afterimage_cannot_draw_gets_one_it_can() {
    let server = start().await;
    server.sign_up().await;
    // openframe is in TRMNL's catalogue and takes WebP images.
    let answer = server.setup_as(&mac(1), "openframe").await;
    let (key, code) = (
        answer["api_key"].as_str().unwrap().to_owned(),
        answer["friendly_id"].as_str().unwrap().to_owned(),
    );
    let (status, body) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_ne!(body["device"]["display"]["model"], "openframe");
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["status"], 0, "{answer}");
}

#[tokio::test]
async fn a_display_that_can_no_longer_be_drawn_falls_back_by_the_panel_size() {
    let server = start().await;
    server.sign_up().await;
    // A model TRMNL's catalogue does not list, as the Seeed kit's firmware reports.
    let model = "xiao_epaper_display";
    let answer = server.setup_as(&mac(1), model).await;
    let key = answer["api_key"].as_str().unwrap().to_owned();
    let code = answer["friendly_id"].as_str().unwrap().to_owned();
    let (status, body) = server
        .call("POST", "/manage/devices", Some(json!({ "code": code })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // A check-in reports the panel, 800 by 480; then the stored display leaves
    // the catalogue, as after a catalogue refresh.
    server.display_as(&key, &READINGS, FIRMWARE, model).await;
    server
        .stored("UPDATE devices SET display_model = 'retired'")
        .await;

    let answer = server.display_as(&key, &READINGS, FIRMWARE, model).await;
    let bytes = download(&server, &answer, &key)
        .await
        .bytes()
        .await
        .unwrap();
    let info = png::Decoder::new(std::io::Cursor::new(bytes.to_vec()))
        .read_info()
        .unwrap()
        .info()
        .clone();
    // TRMNL's 800×480 hardware with the most grays, og_plus at 2 bits, rather
    // than the 1-bit og_png that a name alone falls back to.
    assert_eq!((info.width, info.height), (800, 480));
    assert_eq!(info.bit_depth, png::BitDepth::Two);
}

async fn set_sleep(server: &common::Server, device: i64, sleep: Value) -> (StatusCode, Value) {
    server
        .call(
            "PUT",
            &format!("/manage/devices/{device}/sleep"),
            Some(json!({ "sleep": sleep })),
        )
        .await
}

async fn device(server: &common::Server, id: i64) -> Value {
    server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await
        .1["device"]
        .clone()
}

/// The harness clock's start, Friday 2027-01-15 08:00 UTC, the server's zone.
const EIGHT: i64 = 1_800_000_000;

#[tokio::test]
async fn a_device_sleeps_through_its_window_and_wakes_where_its_playlist_was() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    play(&server, id, &[first, second]).await;
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["showing"]["screenId"], first);

    let (status, body) = set_sleep(
        &server,
        id,
        json!({ "windows": [{ "from": "08:00", "until": "09:00" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["device"]["sleep"],
        json!({ "windows": [{ "from": "08:00", "until": "09:00" }] })
    );

    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["refresh_rate"], 60 * 60, "sleeps until 09:00");
    assert_eq!(
        download(&server, &answer, &key).await.status(),
        StatusCode::OK
    );
    let asleep = device(&server, id).await;
    assert_eq!(asleep["showing"]["asleep"], true);
    assert!(asleep["showing"].get("screenId").is_none(), "{asleep}");
    assert_eq!(asleep["wakesAt"], EIGHT + 60 * 60);

    server.clock.advance(60 * 60);
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["refresh_rate"], 15 * 60);
    let awake = device(&server, id).await;
    assert_eq!(awake["showing"]["asleep"], false);
    assert_eq!(
        awake["showing"]["screenId"], second,
        "the playlist goes on from the screen shown before it slept"
    );
}

#[tokio::test]
async fn an_awake_device_wakes_in_time_to_fall_asleep() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let first = screen(
        &server,
        "First",
        json!({ "content": { "title": { "text": "First" } } }),
    )
    .await;
    play(&server, id, &[first]).await;
    set_sleep(
        &server,
        id,
        json!({ "windows": [{ "days": ["friday"], "from": "08:10", "until": "07:00" }] }),
    )
    .await;
    assert_eq!(
        server.display(&key, &READINGS).await["refresh_rate"],
        10 * 60,
        "wakes at 08:10 to fall asleep, before its quarter hour is up"
    );
    server.clock.advance(10 * 60);
    assert_eq!(
        server.display(&key, &READINGS).await["refresh_rate"],
        (22 * 60 + 50) * 60,
        "Friday's window runs past midnight to Saturday 07:00"
    );
}

#[tokio::test]
async fn a_sleep_screen_shows_while_asleep_and_gives_way_when_it_fails() {
    let source = Source::default();
    let url = source.serve().await;
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let night = screen(
        &server,
        "Night",
        json!({
            "sources": { "sky": { "url": url } },
            "content": { "title": { "text": "{{ sky.moon }}" } }
        }),
    )
    .await;
    source.set(Some(json!({ "moon": "Waxing" })));
    set_sleep(&server, id, json!({ "windows": [{}], "screenId": night })).await;

    server.display(&key, &READINGS).await;
    let shown = device(&server, id).await;
    assert_eq!(shown["showing"]["screenId"], night);
    assert_eq!(shown["showing"]["asleep"], true);

    source.set(None);
    server.display(&key, &READINGS).await;
    let shown = device(&server, id).await;
    assert!(
        shown["showing"].get("screenId").is_none(),
        "the server's own sleep screen: {shown}"
    );
    assert_eq!(shown["showing"]["asleep"], true);
}

#[tokio::test]
async fn sleep_takes_only_the_accounts_screens_and_loses_a_deleted_one() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, _) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    assert_eq!(
        device(&server, id).await["sleep"],
        json!({ "windows": [] }),
        "a new device never sleeps"
    );

    let eve = server.second_account().await;
    let theirs = screen(
        &eve,
        "Theirs",
        json!({ "content": { "title": { "text": "Theirs" } } }),
    )
    .await;
    let (status, _) = set_sleep(&server, id, json!({ "windows": [], "screenId": theirs })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let night = screen(
        &server,
        "Night",
        json!({ "content": { "title": { "text": "Night" } } }),
    )
    .await;
    let windows = json!([
        { "days": ["saturday", "sunday"], "from": "23:30", "until": "09:00" },
        { "days": ["monday", "tuesday", "wednesday", "thursday", "friday"], "from": "23:00", "until": "07:00" }
    ]);
    let (status, body) = set_sleep(
        &server,
        id,
        json!({ "windows": windows, "screenId": night }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        device(&server, id).await["sleep"],
        json!({ "windows": windows, "screenId": night })
    );

    let (status, _) = server
        .call("DELETE", &format!("/manage/screens/{night}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        device(&server, id).await["sleep"],
        json!({ "windows": windows }),
        "the server's own screen stands in"
    );
}

#[tokio::test]
async fn a_device_that_wakes_into_a_failing_screen_says_so_rather_than_that_it_sleeps() {
    let source = Source::default();
    let url = source.serve().await;
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let weather = screen(&server, "Weather", weather(&url)).await;
    source.set(Some(json!({ "temperature": 4, "hourly": [1, 2, 3] })));
    play(&server, id, &[weather]).await;
    set_sleep(
        &server,
        id,
        json!({ "windows": [{ "from": "08:00", "until": "09:00" }] }),
    )
    .await;
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["showing"]["asleep"], true);

    source.set(None);
    server.clock.advance(60 * 60);
    server.display(&key, &READINGS).await;
    let awake = device(&server, id).await;
    assert_eq!(
        awake["showing"]["asleep"], false,
        "the sleep image does not stand through the day: {awake}"
    );
    assert!(awake["showing"].get("screenId").is_none(), "{awake}");
}

#[tokio::test]
async fn only_the_devices_account_sets_its_sleep() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, _) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let eve = server.second_account().await;
    let (status, _) = set_sleep(&eve, id, json!({ "windows": [{}] })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(device(&server, id).await["sleep"], json!({ "windows": [] }));
}

#[tokio::test]
async fn a_device_set_up_again_waits_for_its_first_wake() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["wakesAt"], EIGHT + 15 * 60);

    let (again, _) = server.claim(&mac(1)).await;
    assert_eq!(again["id"], id, "the same device, set up again");
    let shown = device(&server, id).await;
    assert!(
        shown.get("wakesAt").is_none(),
        "the old setup's wake does not stand for the new one: {shown}"
    );
}

#[tokio::test]
async fn the_current_screen_counts_down_to_the_wake_the_check_in_asked_for() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    set_sleep(
        &server,
        id,
        json!({ "windows": [{ "from": "08:00", "until": "09:00" }] }),
    )
    .await;
    server.display(&key, &READINGS).await;

    server.clock.advance(20 * 60);
    assert_eq!(
        server.current(&key).await["refresh_rate"],
        40 * 60,
        "forty minutes of its sleep are left"
    );
    server.clock.advance(50 * 60);
    assert_eq!(
        server.current(&key).await["refresh_rate"],
        15 * 60,
        "past the wake, its refresh interval"
    );
}

async fn update(server: &common::Server, id: i64, changes: Value) -> (StatusCode, Value) {
    server
        .call("PATCH", &format!("/manage/devices/{id}"), Some(changes))
        .await
}

#[tokio::test]
async fn a_pinned_screen_stands_in_for_the_schedule_until_set_back() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let second = screen(&server, "Second", text("Second")).await;
    let guests = screen(&server, "Guests", text("Wi-Fi")).await;
    play(&server, id, &[first, second]).await;
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["showing"]["screenId"], first);

    let (status, body) = update(&server, id, json!({ "shows": { "screenId": guests } })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device"]["shows"], json!({ "screenId": guests }));
    for _ in 0..2 {
        server.display(&key, &READINGS).await;
        let shown = device(&server, id).await;
        assert_eq!(shown["showing"]["screenId"], guests);
        assert!(shown["showing"].get("playlistId").is_none(), "{shown}");
    }

    update(&server, id, json!({ "shows": { "schedule": {} } })).await;
    server.display(&key, &READINGS).await;
    assert_eq!(
        device(&server, id).await["showing"]["screenId"],
        second,
        "the playlist goes on from where the pin found it"
    );

    update(&server, id, json!({ "shows": { "screenId": guests } })).await;
    server
        .call("DELETE", &format!("/manage/screens/{guests}"), None)
        .await;
    assert_eq!(
        device(&server, id).await["shows"],
        json!({ "schedule": {} }),
        "deleting the pinned screen sets the device back to its schedule"
    );

    let eve = server.second_account().await;
    let theirs = screen(&eve, "Theirs", text("Theirs")).await;
    let (status, _) = update(&server, id, json!({ "shows": { "screenId": theirs } })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_pinned_playlist_stands_in_for_the_schedule_until_set_back() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let text = |label: &str| json!({ "content": { "title": { "text": label } } });
    let first = screen(&server, "First", text("First")).await;
    let guest_wifi = screen(&server, "Wi-Fi", text("Wi-Fi")).await;
    let guest_map = screen(&server, "Map", text("Map")).await;
    play(&server, id, &[first]).await;
    let guests = playlist(&server, "Guests", &[guest_wifi, guest_map]).await;
    // A slot from 08:05 does not wake a pinned device: it follows no schedule.
    let usual = device(&server, id).await["schedule"]["playlistId"].clone();
    let (status, body) = set_schedule(
        &server,
        id,
        json!({ "playlistId": usual, "slots": [{ "playlistId": usual, "from": "08:05", "until": "09:00" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = update(&server, id, json!({ "shows": { "playlistId": guests } })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device"]["shows"], json!({ "playlistId": guests }));
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(
        answer["refresh_rate"],
        15 * 60,
        "its refresh, not the slot's start"
    );
    let shown = device(&server, id).await;
    assert_eq!(shown["showing"]["screenId"], guest_wifi);
    assert_eq!(shown["showing"]["playlistId"], guests);
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["showing"]["screenId"], guest_map);

    server
        .call("DELETE", &format!("/manage/playlists/{guests}"), None)
        .await;
    assert_eq!(
        device(&server, id).await["shows"],
        json!({ "schedule": {} }),
        "deleting the pinned playlist sets the device back to its schedule"
    );
    server.display(&key, &READINGS).await;
    assert_eq!(device(&server, id).await["showing"]["screenId"], first);

    let eve = server.second_account().await;
    let theirs = playlist(&eve, "Theirs", &[]).await;
    let (status, _) = update(&server, id, json!({ "shows": { "playlistId": theirs } })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = set_schedule(&server, id, json!({ "playlistId": theirs, "slots": [] })).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "nor schedule another account's playlist"
    );
}

#[tokio::test]
async fn a_device_held_asleep_checks_in_hourly_until_let_go() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let first = screen(
        &server,
        "First",
        json!({ "content": { "title": { "text": "First" } } }),
    )
    .await;
    play(&server, id, &[first]).await;

    let (status, body) = update(&server, id, json!({ "sleepHeld": true })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device"]["sleepHeld"], true);
    assert_eq!(
        server.display(&key, &READINGS).await["refresh_rate"],
        60 * 60
    );
    let held = device(&server, id).await;
    assert_eq!(held["showing"]["asleep"], true);
    assert!(held["showing"].get("screenId").is_none(), "{held}");

    update(&server, id, json!({ "sleepHeld": false })).await;
    server.clock.advance(60 * 60);
    assert_eq!(
        server.display(&key, &READINGS).await["refresh_rate"],
        15 * 60
    );
    let awake = device(&server, id).await;
    assert_eq!(awake["showing"]["asleep"], false);
    assert_eq!(awake["showing"]["screenId"], first);
}

#[tokio::test]
async fn a_device_held_asleep_names_no_wake_even_in_a_window() {
    let server = start().await;
    server.sign_up().await;
    let (claimed, key) = server.claim(&mac(1)).await;
    let id = claimed["id"].as_i64().unwrap();
    let image = || async {
        server
            .http
            .get(format!("{}/manage/devices/{id}/image", server.url))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
    };

    update(&server, id, json!({ "sleepHeld": true })).await;
    server.display(&key, &READINGS).await;
    let held = image().await;

    // The clock stands at 08:00, an hour before this window ends.
    let (status, body) = set_sleep(
        &server,
        id,
        json!({ "windows": [{ "from": "08:00", "until": "09:00" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    server.display(&key, &READINGS).await;
    assert_eq!(
        image().await,
        held,
        "held in a window, it names no wake it would not keep"
    );

    update(&server, id, json!({ "sleepHeld": false })).await;
    assert_eq!(
        server.display(&key, &READINGS).await["refresh_rate"],
        60 * 60,
        "it sleeps until the window ends"
    );
    assert_ne!(
        image().await,
        held,
        "in a window alone, it says when it wakes"
    );
}
