//! P0 任务 ↔ 活动关联失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /tasks/{id}` 支持 `eventId`（null 解绑）；
//! - 任务 DTO 返回 `eventId`；
//! - `GET /boards/{id}/tasks?eventId=` 只返回该活动的卡片。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn setup(app: &TestApp, token: &str) -> (String, String) {
    let project = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "活动项目", "description": "" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let board = request(
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
        .to_string();
    let column = request(
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
        .to_string();
    (board, column)
}

async fn create_task(app: &TestApp, token: &str, column: &str, title: &str) -> String {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": title })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn board_task_ids(app: &TestApp, token: &str, board: &str, query: &str) -> Vec<String> {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks{query}"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
    .as_array()
    .unwrap()
    .iter()
    .map(|row| row["id"].as_str().unwrap().to_string())
    .collect()
}

#[tokio::test]
async fn link_task_to_event_and_filter() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (board, column) = setup(&app, &token).await;
    let event_id = Uuid::now_v7();

    let linked = create_task(&app, &token, &column, "活动筹备卡片").await;
    let other = create_task(&app, &token, &column, "普通卡片").await;

    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{linked}"),
        Some(&token),
        Some(&json!({ "eventId": event_id })),
    )
    .await;
    let updated: Value = updated.expect(StatusCode::OK);
    assert_eq!(updated["eventId"], event_id.to_string());

    let ids = board_task_ids(&app, &token, &board, &format!("?eventId={event_id}")).await;
    assert_eq!(ids, vec![linked.clone()]);

    // 列表 DTO 也带 eventId
    let all = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks"),
        Some(&token),
        None,
    )
    .await;
    let all = all.expect(StatusCode::OK);
    let row = all
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == linked)
        .unwrap();
    assert_eq!(row["eventId"], event_id.to_string());

    // 解绑
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{linked}"),
        Some(&token),
        Some(&json!({ "eventId": null })),
    )
    .await
    .expect(StatusCode::OK);
    let ids = board_task_ids(&app, &token, &board, &format!("?eventId={event_id}")).await;
    assert!(ids.is_empty());

    let _ = other;
}
