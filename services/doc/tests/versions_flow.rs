//! P1 版本内容读取失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /spaces/{id}/nodes/{nodeId}/versions/{version}` 返回该版本正文与作者；
//! - 版本不存在 404；非空间成员 403；
//! - 每次保存会把保存前的内容快照为当前版本号（既有行为）。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "版本库", "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_page(app: &TestApp, token: &str, space_id: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&json!({ "kind": "page", "title": "版本页面" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn put(app: &TestApp, token: &str, space_id: &str, node_id: &str, md: &str, base: i64) {
    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node_id}/content"),
        Some(token),
        Some(&json!({ "contentMd": md, "baseVersion": base })),
    )
    .await
    .expect(StatusCode::OK);
}

#[tokio::test]
async fn read_specific_version_content() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;
    let page = create_page(&app, &token, &space_id).await;

    put(&app, &token, &space_id, &page, "第一版", 1).await;
    put(&app, &token, &space_id, &page, "第二版", 2).await;
    put(&app, &token, &space_id, &page, "第三版", 3).await;

    // 保存前的快照：v2 = 第二版保存前的内容（第一版）
    let version = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/versions/2"),
        Some(&token),
        None,
    )
    .await;
    let body = version.expect(StatusCode::OK);
    assert_eq!(body["version"], 2);
    assert_eq!(body["contentMd"], "第一版");
    assert_eq!(body["authorId"], user.to_string());

    // 不存在的版本
    let missing = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/versions/99"),
        Some(&token),
        None,
    )
    .await;
    missing.expect(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn version_read_requires_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    let page = create_page(&app, &owner_token, &space_id).await;
    put(&app, &owner_token, &space_id, &page, "内容", 1).await;

    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/versions/1"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
