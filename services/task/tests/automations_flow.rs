//! P2 自动化规则失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET/POST /boards/{id}/automations`、`DELETE /automations/{id}`（看板 admin 管理）；
//! - trigger ∈ task.completed / task.created；action ∈ add_label / notify_assignees；
//! - add_label 需同看板标签；完成卡片时自动打标签/通知负责人（排除操作者）；
//! - 新建卡片时 task.created 规则同样生效；非法值 422；viewer 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn setup(app: &TestApp, token: &str) -> (String, String, String) {
    let project = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "自动化项目", "description": "" })),
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
    (project, board, column)
}

async fn create_label(app: &TestApp, token: &str, board: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(token),
        Some(&json!({ "name": "已完成", "color": "#22c55e" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_rule(app: &TestApp, token: &str, board: &str, body: Value) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/automations"),
        Some(token),
        Some(&body),
    )
    .await
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

#[tokio::test]
async fn completed_rule_adds_label_and_notifies() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let assignee = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let (project, board, column) = setup(&app, &owner_token).await;
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(&owner_token),
        Some(&json!({ "userIds": [assignee] })),
    )
    .await
    .expect(StatusCode::OK);

    let label = create_label(&app, &owner_token, &board).await;
    create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.completed", "action": "add_label", "labelId": label }),
    )
    .await
    .expect(StatusCode::CREATED);
    create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.completed", "action": "notify_assignees" }),
    )
    .await
    .expect(StatusCode::CREATED);

    let task = create_task(
        &app,
        &owner_token,
        &column,
        json!({ "title": "发布招新页", "assigneeIds": [assignee] }),
    )
    .await;

    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/status"),
        Some(&owner_token),
        Some(&json!({ "status": "done" })),
    )
    .await
    .expect(StatusCode::OK);

    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&owner_token),
        None,
    )
    .await;
    let detail = detail.expect(StatusCode::OK);
    assert!(detail["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == label));

    // 通知事件
    let rows = club_bus::outbox::fetch_due(&app.state.db, 200, app.state.now())
        .await
        .expect("outbox");
    let event = rows
        .into_iter()
        .find(|row| row.event_type == "task.rule_triggered")
        .expect("应产生自动化通知")
        .payload;
    let targets: Vec<String> = event["targetUsers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    assert_eq!(targets, vec![assignee.to_string()]);
}

#[tokio::test]
async fn created_rule_and_validation() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let (project, board, column) = setup(&app, &owner_token).await;
    let label = create_label(&app, &owner_token, &board).await;

    create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.created", "action": "add_label", "labelId": label }),
    )
    .await
    .expect(StatusCode::CREATED);

    let task = create_task(&app, &owner_token, &column, json!({ "title": "新卡片" })).await;
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
        .iter()
        .any(|row| row["id"] == label));

    // 非法 trigger / action
    let bad_trigger = create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.updated", "action": "add_label", "labelId": label }),
    )
    .await;
    assert_eq!(
        bad_trigger.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );
    let bad_action = create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.completed", "action": "run_script" }),
    )
    .await;
    assert_eq!(
        bad_action.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );
    // add_label 缺 labelId
    let missing = create_rule(
        &app,
        &owner_token,
        &board,
        json!({ "trigger": "task.completed", "action": "add_label" }),
    )
    .await;
    assert_eq!(
        missing.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // viewer 不能建规则
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(&owner_token),
        Some(&json!({ "userIds": [viewer] })),
    )
    .await
    .expect(StatusCode::OK);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(&owner_token),
        Some(&json!({ "userId": viewer, "role": "viewer" })),
    )
    .await
    .expect(StatusCode::CREATED);
    create_rule(
        &app,
        &viewer_token,
        &board,
        json!({ "trigger": "task.created", "action": "notify_assignees" }),
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 列表 + 删除
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/automations"),
        Some(&owner_token),
        None,
    )
    .await;
    let list = list.expect(StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let rule_id = list[0]["id"].as_str().unwrap().to_string();
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/automations/{rule_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/automations"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(list.expect(StatusCode::OK).as_array().unwrap().is_empty());
}
