//! The web UI's files as the server sends them to browsers.

mod common;

use std::sync::Arc;

use afterimage_server::keys::OsEntropy;
use reqwest::StatusCode;
use reqwest::header::{CACHE_CONTROL, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use tempfile::TempDir;

/// A built web UI: its shell and one hashed asset.
fn web_dir() -> TempDir {
    let web = TempDir::new().unwrap();
    std::fs::write(web.path().join("index.html"), "<div id=\"root\"></div>").unwrap();
    std::fs::create_dir(web.path().join("assets")).unwrap();
    std::fs::write(web.path().join("assets/index-Bx1a2b3c.js"), "export {};").unwrap();
    web
}

#[tokio::test]
async fn the_shell_is_sent_whole_even_to_a_browser_holding_validators() {
    let web = web_dir();
    let server = common::start_with(
        Arc::new(OsEntropy),
        &["--web-dir", web.path().to_str().unwrap()],
    )
    .await;
    for path in ["/", "/devices/1"] {
        let response = server
            .http
            .get(format!("{}{path}", server.url))
            .header(IF_MODIFIED_SINCE, "Thu, 01 Jan 2099 00:00:00 GMT")
            .header(IF_NONE_MATCH, "*")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.headers()[CACHE_CONTROL], "no-cache", "{path}");
        assert!(response.headers().get(ETAG).is_none(), "{path}");
        assert!(response.headers().get(LAST_MODIFIED).is_none(), "{path}");
        assert_eq!(response.text().await.unwrap(), "<div id=\"root\"></div>");
    }
}

#[tokio::test]
async fn hashed_assets_are_kept_and_a_missing_one_is_not_the_shell() {
    let web = web_dir();
    let server = common::start_with(
        Arc::new(OsEntropy),
        &["--web-dir", web.path().to_str().unwrap()],
    )
    .await;
    let asset = server
        .http
        .get(format!("{}/assets/index-Bx1a2b3c.js", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(
        asset.headers()[CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    let missing = server
        .http
        .get(format!("{}/assets/index-gone.js", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
