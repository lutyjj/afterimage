//! The MCP endpoint at `/mcp`: the operations the model tags `mcp` as tools,
//! called with a personal API token and answered by the management API.

mod common;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{Server, mac, start};
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Creates a token through the signed-in session and returns its secret.
async fn token(server: &Server, body: Value) -> String {
    let (status, created) = server
        .call("POST", "/manage/account/tokens", Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created["secret"].as_str().unwrap().to_owned()
}

/// One stateless request, as a 2026-07-28 client sends it: no handshake, the
/// protocol version and the client named on the request itself.
async fn ask(
    client: &reqwest::Client,
    server: &Server,
    secret: Option<&str>,
    method: &str,
    mut params: Value,
) -> (StatusCode, Option<String>, Value) {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let mut request = client
        .post(format!("{}/mcp", server.url))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", method)
        .body(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string());
    if let Some(name) = params.get("name").and_then(Value::as_str) {
        request = request.header("Mcp-Name", name);
    }
    if let Some(secret) = secret {
        request = request.bearer_auth(secret);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let challenge = response
        .headers()
        .get("www-authenticate")
        .map(|value| value.to_str().unwrap().to_owned());
    let body = response.bytes().await.unwrap();
    (
        status,
        challenge,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

fn plain() -> reqwest::Client {
    reqwest::Client::builder()
        .tls_certs_only([])
        .build()
        .unwrap()
}

/// A tool call's result.
async fn call(server: &Server, secret: &str, tool: &str, arguments: Value) -> Value {
    let (status, _, body) = ask(
        &plain(),
        server,
        Some(secret),
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["result"].clone()
}

#[tokio::test]
async fn only_a_token_the_api_accepts_opens_the_endpoint() {
    let server = start().await;
    server.sign_up().await;

    let (status, challenge, _) = ask(&plain(), &server, None, "tools/list", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(challenge.as_deref(), Some("Bearer"));

    // A browser's session stays with the browser: the endpoint never reads a
    // cookie.
    let (status, _, _) = ask(&server.http, &server, None, "tools/list", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, challenge, _) = ask(
        &plain(),
        &server,
        Some("not-a-token"),
        "tools/list",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(challenge.as_deref(), Some("Bearer error=\"invalid_token\""));
}

#[tokio::test]
async fn the_tools_are_the_operations_the_model_tags() {
    let server = start().await;
    server.sign_up().await;
    let secret = token(&server, json!({ "name": "Agent" })).await;

    let (status, _, body) = ask(&plain(), &server, Some(&secret), "tools/list", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    let named = |name: &str| tools.iter().find(|tool| tool["name"] == name);

    let preview = named("PreviewScreen").expect("PreviewScreen is a tool");
    assert_eq!(preview["annotations"]["readOnlyHint"], false);
    let input = &preview["inputSchema"];
    assert_eq!(input["required"], json!(["display", "document"]));
    let document = input["properties"]["document"]["$ref"].as_str().unwrap();
    assert!(
        input["$defs"][document.trim_start_matches("#/$defs/")].is_object(),
        "a tool's schema carries what it refers to"
    );
    assert_eq!(
        named("ListDevices").unwrap()["annotations"]["readOnlyHint"],
        true
    );
    assert_eq!(
        named("DeleteScreen").unwrap()["annotations"]["destructiveHint"],
        true
    );
    for untagged in [
        "SignOut",
        "CreateAccessToken",
        "UpdateMqttSettings",
        "DeleteDevice",
    ] {
        assert!(named(untagged).is_none(), "{untagged} is not a tool");
    }
}

#[tokio::test]
async fn a_call_is_the_api_answering_its_caller() {
    let server = start().await;
    server.sign_up().await;
    let (device, _) = server.claim(&mac(1)).await;
    let secret = token(&server, json!({ "name": "Agent" })).await;

    let result = call(
        &server,
        &secret,
        "GetDevice",
        json!({ "deviceId": device["id"] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result}");
    let answer: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(answer["device"]["mac"], mac(1));

    let result = call(&server, &secret, "GetDevice", json!({ "deviceId": 999 })).await;
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("HTTP 404"),
        "{result}"
    );

    let result = call(&server, &secret, "GetDevice", json!({ "id": 1 })).await;
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["content"][0]["text"],
        "`id` is not an input of GetDevice"
    );
}

#[tokio::test]
async fn a_preview_answers_its_image_as_an_image() {
    let server = start().await;
    server.sign_up().await;
    let secret = token(&server, json!({ "name": "Agent" })).await;

    let result = call(
        &server,
        &secret,
        "PreviewScreen",
        json!({
            "display": { "model": "og_png", "palette": "bw" },
            "document": { "content": { "title": { "text": "Hello" } } },
        }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result}");
    let content = result["content"].as_array().unwrap();
    let rest: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(rest, json!({ "sources": {}, "problems": [] }));
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["mimeType"], "image/png");
    let png = STANDARD
        .decode(content[1]["data"].as_str().unwrap())
        .unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let info = decoder.read_info().unwrap();
    assert_eq!(
        (info.info().width, info.info().height),
        (800, 480),
        "the display's size"
    );
}

#[tokio::test]
async fn a_read_only_token_reads_and_is_refused_a_change() {
    let server = start().await;
    server.sign_up().await;
    let secret = token(&server, json!({ "name": "Reader", "scope": "read_only" })).await;

    let result = call(&server, &secret, "ListScreens", json!({})).await;
    assert_eq!(result["isError"], false, "{result}");

    let result = call(
        &server,
        &secret,
        "CreateScreen",
        json!({ "name": "Clock", "document": { "content": { "title": { "text": "{{ now() }}" } } } }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("HTTP 403"),
        "{result}"
    );
}
