//! Firmware updates as a device and the web UI meet them: TRMNL's boards, an
//! install request, the check-in answer that carries it, the image the device
//! downloads, and the check-in that shows whether it installed.

mod common;

use std::fmt::Write;
use std::sync::atomic::Ordering;
use std::time::Duration;

use common::{READINGS, Server, mac, start};
use esp_idf_part::{AppType, DataType, Flags, Partition, PartitionTable, SubType, Type};
use reqwest::StatusCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// An app image of one segment holding `payload`, in ESP-IDF's format: the
/// header with `hash_appended` set, the segment, the checksum on the last byte
/// of its 16-byte block, and the SHA-256 of everything before.
fn app_image(payload: &[u8]) -> Vec<u8> {
    let mut image = vec![0; 24];
    image[0] = 0xE9;
    image[1] = 1;
    image[23] = 1;
    image.extend_from_slice(&0x3c00_0020_u32.to_le_bytes());
    image.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    image.extend_from_slice(payload);
    let checksum = payload.iter().fold(0xEF, |checksum, byte| checksum ^ byte);
    image.resize((image.len() + 1).next_multiple_of(16) - 1, 0);
    image.push(checksum);
    let digest = Sha256::digest(&image);
    image.extend_from_slice(&digest);
    image
}

/// A flash image with `app` in `ota_0` at 0x10000.
fn flash_image(app: &[u8]) -> Vec<u8> {
    let partition = |name: &str, ty, subtype, offset, size| {
        Partition::new(name, ty, subtype, offset, size, Flags::empty())
    };
    let table = PartitionTable::new(vec![
        partition(
            "nvs",
            Type::Data,
            SubType::Data(DataType::Nvs),
            0x9000,
            0x5000,
        ),
        partition(
            "otadata",
            Type::Data,
            SubType::Data(DataType::Ota),
            0xe000,
            0x2000,
        ),
        partition(
            "app0",
            Type::App,
            SubType::App(AppType::Ota_0),
            0x10000,
            0x10000,
        ),
        partition(
            "app1",
            Type::App,
            SubType::App(AppType::Ota_1),
            0x20000,
            0x10000,
        ),
    ]);
    let mut flash = vec![0xFF; 0x8000];
    flash.extend_from_slice(&table.to_bin().unwrap());
    flash.resize(0x10000, 0xFF);
    flash.extend_from_slice(app);
    flash
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Publishes two boards on the stand-in TRMNL: `xteink_x4`, whose name is the
/// model its firmware reports, and the Seeed kit, whose firmware reports
/// `xiao_epaper_display`. Returns the kit's 1.8.18 app image.
fn publish(server: &Server) -> Vec<u8> {
    let app = app_image(b"firmware 1.8.18 for the kit");
    let kit = server.trmnl.publish("kit-1.8.18.bin", flash_image(&app));
    let older = server
        .trmnl
        .publish("kit-1.7.8.bin", flash_image(&app_image(b"firmware 1.7.8")));
    let x4 = server
        .trmnl
        .publish("x4.bin", flash_image(&app_image(b"x4")));
    server.trmnl.set_list(Some(json!({ "data": { "models": [
        { "keyname": "seeed_OG_DIY_Kit", "chipFamily": "ESP32-S3",
          "label": "Seeed Studio (TRMNL 7.5\" OG DIY Kit)",
          "versions": [
              { "version": "FW1.8.18", "url": kit },
              { "version": "FW1.7.8-sensors", "url": older },
              { "version": "FW1.7.8", "url": older } ] },
        { "keyname": "xteink_x4", "chipFamily": "ESP32-C3", "label": "Xteink X4",
          "versions": [
              { "version": "FW1.8.18-push-to-refresh", "url": x4 },
              { "version": "FW1.8.9", "url": x4 } ] }
    ] } })));
    app
}

/// A claimed kit on firmware 1.6.10 with its board chosen; returns its id and
/// key.
async fn kit(server: &Server) -> (i64, String) {
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    server
        .display_as(&key, &READINGS, "1.6.10", "xiao_epaper_display")
        .await;
    let (status, body) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "seeed_OG_DIY_Kit" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (id, key)
}

async fn firmware_of(server: &Server, id: i64) -> Value {
    let (status, body) = server
        .call("GET", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["device"]["firmware"].clone()
}

async fn install(server: &Server, id: i64, body: Value) -> (StatusCode, Value) {
    server
        .call(
            "POST",
            &format!("/manage/devices/{id}/firmware-update"),
            Some(body),
        )
        .await
}

#[tokio::test]
async fn lists_trmnls_boards_with_their_newest_releases() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    let (status, body) = server.call("GET", "/manage/firmware-boards", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["boards"],
        json!([
            { "name": "seeed_OG_DIY_Kit", "chipFamily": "ESP32-S3",
              "label": "Seeed Studio (TRMNL 7.5\" OG DIY Kit)", "latestVersion": "1.8.18" },
            { "name": "xteink_x4", "chipFamily": "ESP32-C3", "label": "Xteink X4",
              "latestVersion": "1.8.9" }
        ]),
        "a variant such as 1.8.18-push-to-refresh is no newest release"
    );
}

#[tokio::test]
async fn a_board_comes_from_a_choice_or_from_the_model_the_firmware_reports() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    assert_eq!(
        device["firmware"],
        json!({ "boardChosen": false, "updateAvailable": false }),
        "TRMNL names no board og"
    );
    server
        .display_as(&key, &READINGS, "1.8.9", "xteink_x4")
        .await;
    assert_eq!(
        firmware_of(&server, id).await,
        json!({ "board": "xteink_x4", "boardChosen": false, "latestVersion": "1.8.9",
                "updateAvailable": false })
    );
    let (status, body) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "seeed_OG_DIY_Kit" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["device"]["firmware"],
        json!({ "board": "seeed_OG_DIY_Kit", "boardChosen": true, "latestVersion": "1.8.18",
                "updateAvailable": true }),
        "1.8.9 is older"
    );
    let (status, body) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "trmnl_x" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn the_next_check_in_carries_the_install_and_the_one_after_tells_the_outcome() {
    let server = start().await;
    let app = publish(&server);
    let (id, key) = kit(&server).await;
    let (status, body) = install(&server, id, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let update = &body["device"]["firmware"]["update"];
    assert_eq!(update["version"], "1.8.18");
    assert_eq!(update["state"], "requested");

    let answer = server
        .display_as(&key, &READINGS, "1.6.10", "xiao_epaper_display")
        .await;
    assert_eq!(answer["status"], 0);
    assert_eq!(answer["update_firmware"], true);
    let url = answer["firmware_url"].as_str().unwrap();
    assert_eq!(
        url,
        format!(
            "{}/api/firmware-images/{}.bin",
            server.url,
            sha256_hex(&app)
        )
    );
    assert!(answer["image_url"].is_string(), "the screen comes too");
    assert_eq!(firmware_of(&server, id).await["update"]["state"], "sent");

    // The firmware sends no headers of its own for the image.
    let response = download(url).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(
        response.headers()["content-length"],
        app.len().to_string().as_str(),
        "Update.begin takes the length"
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), app.as_slice());

    let answer = server
        .display_as(&key, &READINGS, "1.8.18", "xiao_epaper_display")
        .await;
    assert_eq!(answer["update_firmware"], false);
    let firmware = firmware_of(&server, id).await;
    assert_eq!(firmware["update"]["state"], "installed");
    assert!(firmware["update"]["finishedAt"].is_number());
    let answer = server
        .display_as(&key, &READINGS, "1.8.18", "xiao_epaper_display")
        .await;
    assert_eq!(answer["update_firmware"], false, "sent once");
}

/// Firmware 1.6 has no pause between failed updates, so the request goes out
/// once and the next check-in decides it.
#[tokio::test]
async fn a_device_that_checks_in_on_its_old_release_did_not_install() {
    let server = start().await;
    publish(&server);
    let (id, key) = kit(&server).await;
    install(&server, id, json!({ "version": "1.7.8" })).await;
    let firmware = firmware_of(&server, id).await;
    assert_eq!(firmware["update"]["version"], "1.7.8");
    for expected in [true, false, false] {
        let answer = server
            .display_as(&key, &READINGS, "1.6.10", "xiao_epaper_display")
            .await;
        assert_eq!(answer["update_firmware"], expected);
    }
    assert_eq!(
        firmware_of(&server, id).await["update"]["state"],
        "not_installed"
    );
}

#[tokio::test]
async fn an_install_needs_a_board_and_a_release_trmnl_lists() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    let (status, body) = install(&server, id, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("board"),
        "{body}"
    );

    let (status, _) = server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "seeed_OG_DIY_Kit" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = install(&server, id, json!({ "version": "1.6.0" })).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, _) = install(&server, 999, json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(firmware_of(&server, id).await["update"].is_null());
}

#[tokio::test]
async fn an_image_that_fails_its_checks_is_never_offered() {
    let server = start().await;
    publish(&server);
    let mut corrupt = flash_image(&app_image(b"firmware 1.8.18 for the kit"));
    // A byte of the segment's data, after the header and the segment header.
    corrupt[0x10000 + 24 + 8 + 3] ^= 0x01;
    server.trmnl.publish("kit-1.8.18.bin", corrupt);
    let (id, key) = kit(&server).await;
    let (status, body) = install(&server, id, json!({})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("checksum"),
        "{body}"
    );
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["update_firmware"], false);
    assert!(firmware_of(&server, id).await["update"].is_null());
}

#[tokio::test]
async fn a_request_can_be_withdrawn_or_replaced_by_another_board() {
    let server = start().await;
    publish(&server);
    let (id, key) = kit(&server).await;
    install(&server, id, json!({})).await;
    let (status, body) = server
        .call(
            "DELETE",
            &format!("/manage/devices/{id}/firmware-update"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["device"]["firmware"]["update"].is_null());
    assert_eq!(
        server.display(&key, &READINGS).await["update_firmware"],
        false
    );

    install(&server, id, json!({})).await;
    server
        .call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "xteink_x4" })),
        )
        .await;
    assert!(
        firmware_of(&server, id).await["update"].is_null(),
        "the kit's image is no image for an X4"
    );
    assert!(
        cached(&server).is_empty(),
        "an image no device may download leaves the cache"
    );
}

#[tokio::test]
async fn an_image_the_cache_lost_is_cut_again_once_before_it_is_sent() {
    let server = start().await;
    let app = publish(&server);
    let (id, key) = kit(&server).await;
    install(&server, id, json!({})).await;
    assert_eq!(server.trmnl.flash_requests.load(Ordering::SeqCst), 1);
    for file in cached(&server) {
        std::fs::remove_file(file).unwrap();
    }
    let hold = server.trmnl.hold_flash().await;
    for _ in 0..3 {
        let answer = server.display(&key, &READINGS).await;
        assert_eq!(answer["update_firmware"], false, "nothing to download yet");
    }
    drop(hold);
    until(|| !cached(&server).is_empty(), "never cut again").await;
    assert_eq!(
        server.trmnl.flash_requests.load(Ordering::SeqCst),
        2,
        "check-ins that find the image missing share one download"
    );
    let answer = server.display(&key, &READINGS).await;
    assert_eq!(answer["update_firmware"], true);
    let image = download(answer["firmware_url"].as_str().unwrap())
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(image.as_ref(), app.as_slice());
    assert_eq!(firmware_of(&server, id).await["update"]["state"], "sent");
}

#[tokio::test]
async fn trmnls_list_is_read_again_only_once_it_is_old() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    let boards = || server.call("GET", "/manage/firmware-boards", None);
    assert_eq!(boards().await.0, StatusCode::OK);
    assert_eq!(boards().await.0, StatusCode::OK);
    assert_eq!(server.trmnl.list_requests.load(Ordering::SeqCst), 1);

    server.trmnl.set_list(None);
    server.clock.advance(6 * 60 * 60);
    let (status, _) = boards().await;
    assert_eq!(status, StatusCode::OK, "the last list stands in");
    until(
        || server.trmnl.list_requests.load(Ordering::SeqCst) == 2,
        "the old list was never read again",
    )
    .await;
    boards().await;
    assert_eq!(
        server.trmnl.list_requests.load(Ordering::SeqCst),
        2,
        "a failed read waits before the next"
    );
}

/// A device read never waits for TRMNL while a list is on hand.
#[tokio::test]
async fn device_reads_answer_while_trmnl_is_slow() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    server.claim(&mac(1)).await;
    assert_eq!(
        server.call("GET", "/manage/devices", None).await.0,
        StatusCode::OK
    );
    server.trmnl.delay_list(Duration::from_secs(60));
    server.clock.advance(6 * 60 * 60);
    let read = tokio::time::timeout(
        Duration::from_secs(5),
        server.call("GET", "/manage/devices", None),
    )
    .await
    .expect("the devices answered while TRMNL was slow");
    assert_eq!(read.0, StatusCode::OK);
    assert_eq!(read.1["devices"][0]["firmware"]["boardChosen"], false);
}

/// The board decides which image a device may get, so a board chosen while an
/// install downloads the old board's image leaves no request behind.
#[tokio::test]
async fn a_board_chosen_during_an_install_wins() {
    let server = start().await;
    publish(&server);
    let (id, key) = kit(&server).await;
    let hold = server.trmnl.hold_flash().await;
    let http = server.http.clone();
    let url = format!("{}/manage/devices/{id}/firmware-update", server.url);
    let installing = tokio::spawn(async move {
        http.post(url)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap()
            .status()
    });
    until(
        || server.trmnl.flash_requests.load(Ordering::SeqCst) == 1,
        "the install never asked TRMNL for its image",
    )
    .await;
    let chosen = tokio::time::timeout(
        Duration::from_secs(5),
        server.call(
            "PATCH",
            &format!("/manage/devices/{id}"),
            Some(json!({ "firmwareBoard": "xteink_x4" })),
        ),
    )
    .await
    .expect("choosing a board does not wait for another's download");
    assert_eq!(chosen.0, StatusCode::OK);
    drop(hold);
    assert_eq!(installing.await.unwrap(), StatusCode::CONFLICT);
    assert!(firmware_of(&server, id).await["update"].is_null());
    assert_eq!(
        server.display(&key, &READINGS).await["update_firmware"],
        false
    );
    assert!(
        cached(&server).is_empty(),
        "the old board's image leaves the cache"
    );
}

/// Firmware 1.6 reads the update fields only on a check-in without a special
/// function (`handleApiDisplayResponse` in `src/bl.cpp`).
#[tokio::test]
async fn a_check_in_for_a_special_function_carries_no_request() {
    let server = start().await;
    publish(&server);
    let (id, key) = kit(&server).await;
    install(&server, id, json!({})).await;
    let answer: Value = server
        .http
        .get(format!("{}/api/display", server.url))
        .header("ID", mac(1))
        .header("Access-Token", &key)
        .header("FW-Version", "1.6.10")
        .header("Model", "xiao_epaper_display")
        .header("special_function", "true")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(answer["status"], 0);
    assert_eq!(answer["update_firmware"], false);
    assert_eq!(
        firmware_of(&server, id).await["update"]["state"],
        "requested"
    );
    let answer = server
        .display_as(&key, &READINGS, "1.6.10", "xiao_epaper_display")
        .await;
    assert_eq!(answer["update_firmware"], true);
}

/// TRMNL names a variant in its release only; the build reports the version it
/// varies (`FW_VERSION_STRING` in `include/config.h`).
#[tokio::test]
async fn a_variant_is_installed_when_the_device_reports_the_version_it_varies() {
    let server = start().await;
    publish(&server);
    server.sign_up().await;
    let (device, key) = server.claim(&mac(1)).await;
    let id = device["id"].as_i64().unwrap();
    server
        .display_as(&key, &READINGS, "1.8.9", "xteink_x4")
        .await;
    let (status, body) = install(&server, id, json!({ "version": "1.8.18-push-to-refresh" })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let answer = server
        .display_as(&key, &READINGS, "1.8.9", "xteink_x4")
        .await;
    assert_eq!(answer["update_firmware"], true);
    server
        .display_as(&key, &READINGS, "1.8.18", "xteink_x4")
        .await;
    assert_eq!(
        firmware_of(&server, id).await["update"]["state"],
        "installed"
    );
}

/// An image stays cached only while a device may still download it.
#[tokio::test]
async fn a_decided_or_removed_request_leaves_no_image_behind() {
    let server = start().await;
    publish(&server);
    let (id, key) = kit(&server).await;
    install(&server, id, json!({})).await;
    server.display(&key, &READINGS).await;
    assert_eq!(cached(&server).len(), 1, "sent and not yet decided");
    server.display(&key, &READINGS).await;
    assert!(cached(&server).is_empty(), "decided");

    install(&server, id, json!({})).await;
    assert_eq!(cached(&server).len(), 1);
    let (status, _) = server
        .call("DELETE", &format!("/manage/devices/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(cached(&server).is_empty(), "removed with its device");
}

#[tokio::test]
async fn without_trmnls_list_there_is_no_board_to_choose() {
    let server = start().await;
    server.trmnl.set_list(None);
    server.sign_up().await;
    let (status, body) = server.call("GET", "/manage/firmware-boards", None).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    let (device, _) = server.claim(&mac(1)).await;
    assert_eq!(
        device["firmware"],
        json!({ "boardChosen": false, "updateAvailable": false })
    );
}

#[tokio::test]
async fn serves_only_firmware_images_it_keeps() {
    let server = start().await;
    let missing = format!("{}/api/firmware-images/{}.bin", server.url, "0".repeat(64));
    assert_eq!(
        download(missing).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let traversal = format!("{}/api/firmware-images/..%2Fafterimage.db", server.url);
    assert_eq!(
        download(traversal).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
}

/// A GET as the firmware sends one for `firmware_url`: no headers of its own and
/// no cookies. The client trusts no certificate, since the build sandbox has no
/// store to load.
async fn download(url: impl reqwest::IntoUrl) -> reqwest::Result<reqwest::Response> {
    reqwest::Client::builder()
        .tls_certs_only([])
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
}

/// Waits for a condition that work the test started, such as a background
/// fetch, brings about.
async fn until(condition: impl Fn() -> bool, failure: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(tokio::time::Instant::now() < deadline, "{failure}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The files in the firmware cache.
fn cached(server: &Server) -> Vec<std::path::PathBuf> {
    match std::fs::read_dir(server.data.path().join("cache/firmware")) {
        Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}
