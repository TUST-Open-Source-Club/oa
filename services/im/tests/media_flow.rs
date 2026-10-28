//! P1 媒体消息的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /api/v1/im/uploads`（JSON，base64）上传图片/附件（≤5MB），返回附件信息与签名 URL；
//! - `GET /api/v1/im/files/{id}?token=` 用签名直接下载（`<img>` 可用，不需要 Authorization）；
//! - 上传需登录；空文件/超大文件 422；签名无效、过期或与文件不匹配 401；
//! - 消息类型 `image`/`file` 的 `content.attachments` 存附件元数据，随消息回读。

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use base64::Engine as _;
use common::*;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// 原始字节响应。
struct RawResponse {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

/// 取原始响应（二进制下载用）。
async fn get_raw(app: &TestApp, uri: &str) -> RawResponse {
    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("请求"),
        )
        .await
        .expect("执行请求");
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("响应体")
        .to_bytes()
        .to_vec();
    RawResponse {
        status,
        content_type,
        body,
    }
}

/// 上传文件。
async fn upload(app: &TestApp, token: &str, name: &str, mime: &str, bytes: &[u8]) -> TestResponse {
    let payload = json!({
        "name": name,
        "mime": mime,
        "dataBase64": base64::engine::general_purpose::STANDARD.encode(bytes),
    });
    request(
        &app.app,
        "POST",
        "/api/v1/im/uploads",
        Some(token),
        Some(&payload),
    )
    .await
}

#[tokio::test]
async fn upload_then_download_with_signed_url() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);

    let bytes = b"\x89PNG\r\n\x1a\n-fake-image";
    let uploaded = upload(&app, &token, "海报.png", "image/png", bytes).await;
    let body = uploaded.expect(StatusCode::CREATED);
    assert_eq!(body["name"], "海报.png");
    assert_eq!(body["mime"], "image/png");
    assert_eq!(body["size"], bytes.len() as i64);
    let url = body["url"].as_str().unwrap();
    assert!(url.contains("token="), "返回签名下载地址：{url}");

    // 直接 GET（不带 Authorization），模拟 <img>
    let raw = get_raw(&app, url).await;
    assert_eq!(raw.status, StatusCode::OK);
    assert_eq!(raw.content_type.as_deref(), Some("image/png"));
    assert_eq!(raw.body, bytes);

    // 签名与文件不匹配 → 401
    let file_id = body["id"].as_str().unwrap();
    let other_id = Uuid::now_v7();
    let swapped = url.replace(file_id, &other_id.to_string());
    let raw = get_raw(&app, &swapped).await;
    assert_eq!(raw.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn upload_requires_auth_and_valid_payload() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);

    // 未登录
    let denied = request(
        &app.app,
        "POST",
        "/api/v1/im/uploads",
        None,
        Some(&json!({ "name": "a.txt", "mime": "text/plain", "dataBase64": "aGk=" })),
    )
    .await;
    denied.expect(StatusCode::UNAUTHORIZED);

    // 空文件
    let empty = upload(&app, &token, "empty.txt", "text/plain", b"").await;
    let body = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // 非法 base64
    let invalid = request(
        &app.app,
        "POST",
        "/api/v1/im/uploads",
        Some(&token),
        Some(&json!({ "name": "x.txt", "mime": "text/plain", "dataBase64": "%%%" })),
    )
    .await;
    let body = invalid.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // 超过 5MB
    let oversized = vec![0u8; im_service::routes::MAX_FILE_BYTES + 1];
    let too_big = upload(
        &app,
        &token,
        "big.bin",
        "application/octet-stream",
        &oversized,
    )
    .await;
    let body = too_big.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // 签名无效
    let raw = get_raw(
        &app,
        &format!("/api/v1/im/files/{}?token=bad", Uuid::now_v7()),
    )
    .await;
    assert_eq!(raw.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn image_message_carries_attachment() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);

    let created = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(&owner_token),
        Some(&json!({ "type": "group", "name": "媒体群", "memberIds": [member] })),
    )
    .await;
    let conversation_id = created.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let attachment = upload(&app, &owner_token, "图片.png", "image/png", b"png-bytes")
        .await
        .expect(StatusCode::CREATED);
    let payload: Value = json!({
        "type": "image",
        "content": { "attachments": [attachment] },
    });
    let sent = request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&owner_token),
        Some(&payload),
    )
    .await;
    let sent = sent.expect(StatusCode::CREATED);
    assert_eq!(sent["type"], "image");

    let history = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&member_token),
        None,
    )
    .await;
    let list = history.expect(StatusCode::OK);
    assert_eq!(list[0]["type"], "image");
    assert_eq!(list[0]["content"]["attachments"][0]["name"], "图片.png");
    assert!(list[0]["content"]["attachments"][0]["url"]
        .as_str()
        .unwrap()
        .contains("token="));
}
