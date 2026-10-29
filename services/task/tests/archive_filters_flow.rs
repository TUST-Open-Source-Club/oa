//! P0 卡片归档与看板筛选失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /tasks/{id}/archive`、`POST /tasks/{id}/restore`；归档后不出现在看板任务里；
//! - `GET /boards/{id}/archived-tasks` 列出归档卡片；
//! - `GET /boards/{id}/tasks` 支持 `assigneeId`、`labelId`、`q`、`due`（overdue/today/week）过滤。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_project(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "归档项目", "description": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn first_board(app: &TestApp, token: &str, project: &str) -> String {
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

async fn first_column(app: &TestApp, token: &str, board: &str) -> String {
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

async fn create_task(app: &TestApp, token: &str, column: &str, body: Value) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&body),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn board_task_ids(app: &TestApp, token: &str, board: &str, query: &str) -> Vec<String> {
    let response = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks{query}"),
        Some(token),
        None,
    )
    .await;
    response
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn archive_hides_and_restores_card() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token).await;
    let board = first_board(&app, &token, &project).await;
    let column = first_column(&app, &token, &board).await;
    let task = create_task(&app, &token, &column, json!({ "title": "要归档" })).await;
    create_task(&app, &token, &column, json!({ "title": "留着的" })).await;

    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/archive"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let ids = board_task_ids(&app, &token, &board, "").await;
    assert_eq!(ids.len(), 1);
    assert!(!ids.contains(&task));

    let archived = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/archived-tasks"),
        Some(&token),
        None,
    )
    .await;
    let archived = archived.expect(StatusCode::OK);
    assert_eq!(archived.as_array().unwrap().len(), 1);
    assert_eq!(archived[0]["id"], task);

    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/restore"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let ids = board_task_ids(&app, &token, &board, "").await;
    assert!(ids.contains(&task));
}

#[tokio::test]
async fn board_filters_by_assignee_label_keyword_and_due() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let project = create_project(&app, &owner_token).await;
    let board = first_board(&app, &owner_token, &project).await;
    let column = first_column(&app, &owner_token, &board).await;

    // 让 member 成为项目成员
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(&owner_token),
        Some(&json!({ "userIds": [member] })),
    )
    .await
    .expect(StatusCode::OK);

    let urgent = create_task(
        &app,
        &owner_token,
        &column,
        json!({ "title": "紧急：场地确认", "assigneeIds": [member] }),
    )
    .await;
    let other = create_task(&app, &owner_token, &column, json!({ "title": "海报设计" })).await;

    // 标签
    let label = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(&owner_token),
        Some(&json!({ "name": "紧急", "color": "#ef4444" })),
    )
    .await;
    let label_id = label.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{urgent}/labels/{label_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    // 负责人过滤
    let ids = board_task_ids(&app, &owner_token, &board, &format!("?assigneeId={member}")).await;
    assert_eq!(ids, vec![urgent.clone()]);

    // 标签过滤
    let ids = board_task_ids(&app, &owner_token, &board, &format!("?labelId={label_id}")).await;
    assert_eq!(ids, vec![urgent.clone()]);

    // 关键词
    let ids = board_task_ids(&app, &owner_token, &board, "?q=%E6%B5%B7%E6%8A%A5").await;
    assert_eq!(ids, vec![other.clone()]);

    // 截止过滤：给 other 一个昨天的时间，overdue 只返回它
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{other}"),
        Some(&owner_token),
        Some(&json!({ "dueAt": yesterday })),
    )
    .await
    .expect(StatusCode::OK);
    let ids = board_task_ids(&app, &owner_token, &board, "?due=overdue").await;
    assert_eq!(ids, vec![other]);
}
