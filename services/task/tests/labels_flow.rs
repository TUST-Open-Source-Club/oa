//! P0 看板标签失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET/POST /boards/{id}/labels`：看板级标签（名称 + 颜色），管理员可建；
//! - `PATCH/DELETE /labels/{id}`：改名/改色/删除；
//! - `POST/DELETE /tasks/{id}/labels/{labelId}`：给卡片打/取消标签；
//! - 任务 DTO 返回 `labels: [{ id, name, color }]`；跨看板标签 422；非成员 403。

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
        Some(&json!({ "name": "标签项目", "description": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn first_board(app: &TestApp, token: &str, project: &str) -> String {
    let response = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(token),
        None,
    )
    .await;
    response.expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn first_column(app: &TestApp, token: &str, board: &str) -> String {
    let response = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/columns"),
        Some(token),
        None,
    )
    .await;
    response.expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_task(app: &TestApp, token: &str, column: &str, title: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": title })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_label(app: &TestApp, token: &str, board: &str, name: &str, color: &str) -> Value {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(token),
        Some(&json!({ "name": name, "color": color })),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

#[tokio::test]
async fn labels_crud_and_card_attach() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token).await;
    let board = first_board(&app, &token, &project).await;
    let column = first_column(&app, &token, &board).await;
    let task = create_task(&app, &token, &column, "带标签的卡片").await;

    let label = create_label(&app, &token, &board, "紧急", "#ef4444").await;
    let label_id = label["id"].as_str().unwrap().to_string();
    assert_eq!(label["name"], "紧急");

    // 打标签
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/labels/{label_id}"),
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
    let detail = detail.expect(StatusCode::OK);
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "紧急");
    assert_eq!(labels[0]["color"], "#ef4444");

    // 看板任务列表也带标签
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks"),
        Some(&token),
        None,
    )
    .await;
    let list = list.expect(StatusCode::OK);
    assert_eq!(list[0]["labels"][0]["id"], label_id);

    // 改标签
    let renamed = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/labels/{label_id}"),
        Some(&token),
        Some(&json!({ "name": "非常紧急", "color": "#f97316" })),
    )
    .await;
    assert_eq!(renamed.expect(StatusCode::OK)["name"], "非常紧急");

    // 取消标签
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{task}/labels/{label_id}"),
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
    assert!(detail.expect(StatusCode::OK)["labels"]
        .as_array()
        .unwrap()
        .is_empty());

    // 删标签
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/labels/{label_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn label_validation_cross_board_and_permissions() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let project = create_project(&app, &owner_token).await;
    let board = first_board(&app, &owner_token, &project).await;
    let column = first_column(&app, &owner_token, &board).await;
    let task = create_task(&app, &owner_token, &column, "卡片").await;
    let label = create_label(&app, &owner_token, &board, "常规", "#22c55e").await;
    let label_id = label["id"].as_str().unwrap().to_string();

    // 非法颜色
    let bad = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(&owner_token),
        Some(&json!({ "name": "坏颜色", "color": "red" })),
    )
    .await;
    assert_eq!(
        bad.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 非成员
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 跨看板标签：另一看板的标签不能打到这个卡片
    let second = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&owner_token),
        Some(&json!({ "name": "第二看板" })),
    )
    .await;
    let second_board = second.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other_label = create_label(&app, &owner_token, &second_board, "别处", "#3b82f6").await;
    let other_label_id = other_label["id"].as_str().unwrap().to_string();
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/labels/{other_label_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        denied.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 删除标签同时清掉卡片关联
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/labels/{label_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/labels/{label_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(detail.expect(StatusCode::OK)["labels"]
        .as_array()
        .unwrap()
        .is_empty());
}
