//! P0 文档图片上传/下载失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /api/v1/doc/uploads`（JSON base64，≤5MB）→ 返回 `{ id, name, mime, size, url }`；
//! - `GET /api/v1/doc/files/{id}?token=` 免 Authorization 下载（`<img>` 可用），
//!   签名长期有效（文档里的图片 URL 会持久化到 Markdown）；
//! - 签名与文件不匹配 / 非法签名 401；空文件与超大文件 422；上传需登录。

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use base64::Engine as _;
use common::*;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

struct RawResponse {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

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

async fn upload(app: &TestApp, token: &str, name: &str, mime: &str, bytes: &[u8]) -> TestResponse {
    let payload = json!({
        "name": name,
        "mime": mime,
        "dataBase64": base64::engine::general_purpose::STANDARD.encode(bytes),
    });
    request(
        &app.app,
        "POST",
        "/api/v1/doc/uploads",
        Some(token),
        Some(&payload),
    )
    .await
}

#[tokio::test]
async fn upload_then_download_image() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);

    let bytes = b"\x89PNG\r\n\x1a\n-doc-image";
    let uploaded = upload(&app, &token, "配图.png", "image/png", bytes).await;
    let body = uploaded.expect(StatusCode::CREATED);
    assert_eq!(body["name"], "配图.png");
    assert_eq!(body["mime"], "image/png");
    assert_eq!(body["size"], bytes.len() as i64);
    let url = body["url"].as_str().unwrap();
    assert!(url.contains("token="));

    let raw = get_raw(&app, url).await;
    assert_eq!(raw.status, StatusCode::OK);
    assert_eq!(raw.content_type.as_deref(), Some("image/png"));
    assert_eq!(raw.body, bytes);

    // 签名与文件 ID 不匹配
    let file_id = body["id"].as_str().unwrap();
    let swapped = url.replace(file_id, &Uuid::now_v7().to_string());
    assert_eq!(
        get_raw(&app, &swapped).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn upload_validates_auth_and_payload() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);

    request(
        &app.app,
        "POST",
        "/api/v1/doc/uploads",
        None,
        Some(&json!({ "name": "a.txt", "mime": "text/plain", "dataBase64": "aGk=" })),
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    let empty = upload(&app, &token, "empty.txt", "text/plain", b"").await;
    assert_eq!(
        empty.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );

    let invalid = request(
        &app.app,
        "POST",
        "/api/v1/doc/uploads",
        Some(&token),
        Some(&json!({ "name": "x.txt", "mime": "text/plain", "dataBase64": "%%%" })),
    )
    .await;
    assert_eq!(
        invalid.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );

    let oversized = vec![0u8; doc_service::routes::MAX_FILE_BYTES + 1];
    let too_big = upload(
        &app,
        &token,
        "big.bin",
        "application/octet-stream",
        &oversized,
    )
    .await;
    assert_eq!(
        too_big.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );
}

#[tokio::test]
async fn download_rejects_bad_token() {
    let app = spawn().await;
    let raw = get_raw(
        &app,
        &format!("/api/v1/doc/files/{}?token=bad", Uuid::now_v7()),
    )
    .await;
    assert_eq!(raw.status, StatusCode::UNAUTHORIZED);

    // 合法签名但文件不存在 → 404
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let uploaded = upload(&app, &token, "x.png", "image/png", b"x").await;
    let body = uploaded.expect(StatusCode::CREATED);
    let url = body["url"].as_str().unwrap();
    let missing = url.replace(body["id"].as_str().unwrap(), &Uuid::now_v7().to_string());
    assert_eq!(
        get_raw(&app, &missing).await.status,
        StatusCode::UNAUTHORIZED
    );
}
