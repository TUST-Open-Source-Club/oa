//! P2 看板泳道设置失败用例（TDD 红阶段）。
//!
//! 约定：看板 `swimlane` ∈ none/assignee/label，默认 none；
//! `PATCH /boards/{id}` 支持只改泳道或同时改名；非法值 422；非管理员 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

async fn setup(app: &TestApp, token: &str) -> (String, String) {
    let project = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "泳道项目", "description": "" })),
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
    (project, board)
}

#[tokio::test]
async fn swimlane_setting() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let (project, board) = setup(&app, &owner_token).await;

    // 默认 none
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(list.expect(StatusCode::OK)[0]["swimlane"], "none");

    // 只改泳道
    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/boards/{board}"),
        Some(&owner_token),
        Some(&json!({ "swimlane": "assignee" })),
    )
    .await;
    let updated = updated.expect(StatusCode::OK);
    assert_eq!(updated["swimlane"], "assignee");
    assert_eq!(updated["name"], "默认看板");

    // 同时改名
    let renamed = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/boards/{board}"),
        Some(&owner_token),
        Some(&json!({ "name": "招新看板", "swimlane": "label" })),
    )
    .await;
    let renamed = renamed.expect(StatusCode::OK);
    assert_eq!(renamed["name"], "招新看板");
    assert_eq!(renamed["swimlane"], "label");

    // 非法值
    let bad = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/boards/{board}"),
        Some(&owner_token),
        Some(&json!({ "swimlane": "owner" })),
    )
    .await;
    assert_eq!(
        bad.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 普通成员不能改（看板 admin 才能）
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(&owner_token),
        Some(&json!({ "userIds": [member] })),
    )
    .await
    .expect(StatusCode::OK);
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/boards/{board}"),
        Some(&member_token),
        Some(&json!({ "swimlane": "none" })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
