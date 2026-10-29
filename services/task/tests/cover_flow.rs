//! P0 卡片封面失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /tasks/{id}/cover` `{ attachmentId }` 设置封面，`DELETE /tasks/{id}/cover` 取消；
//! - 任务 DTO 返回 `coverAttachmentId` 与带签名的 `coverUrl`；
//! - `GET /attachments/{id}/file?token=` 免鉴权读取（签名匹配），非法签名 401；
//! - 附件必须属于该任务（422）。

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

async fn create_project(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "封面项目", "description": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn board_of(app: &TestApp, token: &str, project: &str) -> String {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn column_of(app: &TestApp, token: &str, board: &str) -> String {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/columns"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_task(app: &TestApp, token: &str, column: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": "封面卡片" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn upload(app: &TestApp, token: &str, task: &str, name: &str, bytes: &[u8]) -> String {
    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/task/tasks/{task}/attachments?name={name}&mime=image/png"
                ))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/octet-stream")
                .body(Body::from(bytes.to_vec()))
                .expect("请求"),
        )
        .await
        .expect("上传");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("体").to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_string()
}

async fn get_raw(app: &TestApp, uri: &str) -> (StatusCode, Vec<u8>) {
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
        .expect("下载");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("体").to_bytes();
    (status, bytes.to_vec())
}

#[tokio::test]
async fn cover_sets_signed_url_and_public_file() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token).await;
    let board = board_of(&app, &token, &project).await;
    let column = column_of(&app, &token, &board).await;
    let task = create_task(&app, &token, &column).await;
    let attachment = upload(&app, &token, &task, "cover.png", b"png-bytes").await;

    let set = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/cover"),
        Some(&token),
        Some(&json!({ "attachmentId": attachment })),
    )
    .await;
    set.expect(StatusCode::OK);

    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&token),
        None,
    )
    .await;
    let detail = detail.expect(StatusCode::OK);
    assert_eq!(detail["coverAttachmentId"], attachment);
    let url = detail["coverUrl"].as_str().unwrap();
    assert!(url.contains("token="), "{url}");

    // 免鉴权签名下载
    let (status, bytes) = get_raw(&app, url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"png-bytes");

    // 非法签名
    let (status, _) = get_raw(
        &app,
        &format!("/api/v1/task/attachments/{attachment}/file?token=bad"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 取消封面
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{task}/cover"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&token),
        None,
    )
    .await;
    assert!(detail.expect(StatusCode::OK)["coverAttachmentId"].is_null());
}

#[tokio::test]
async fn cover_rejects_foreign_attachment() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token).await;
    let board = board_of(&app, &token, &project).await;
    let column = column_of(&app, &token, &board).await;
    let task = create_task(&app, &token, &column).await;
    let other = create_task(&app, &token, &column).await;
    let foreign = upload(&app, &token, &other, "other.png", b"x").await;

    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/cover"),
        Some(&token),
        Some(&json!({ "attachmentId": foreign })),
    )
    .await;
    assert_eq!(
        denied.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );
}
