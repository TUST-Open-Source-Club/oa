//! P1 卡片模板失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET/POST /boards/{id}/templates`、`DELETE /templates/{id}`；
//! - `POST /columns/{id}/tasks/from-template` `{ templateId, title? }` 用模板建卡；
//! - 模板属于看板；跨看板 422；viewer 不能建模板/建卡；空名称 422。

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
        Some(&json!({ "name": "模板项目", "description": "" })),
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

async fn add_member(app: &TestApp, token: &str, project: &str, user: Uuid) {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(token),
        Some(&json!({ "userIds": [user] })),
    )
    .await
    .expect(StatusCode::OK);
}

async fn create_template(app: &TestApp, token: &str, board: &str, name: &str) -> Value {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/templates"),
        Some(token),
        Some(&json!({
            "name": name,
            "title": "排查线上问题",
            "descriptionMd": "步骤：1) 复现 2) 定位 3) 修复",
        })),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

#[tokio::test]
async fn create_use_and_delete_template() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let (_, board, column) = setup(&app, &owner_token).await;

    let template = create_template(&app, &owner_token, &board, "故障模板").await;
    let template_id = template["id"].as_str().unwrap().to_string();
    assert_eq!(template["name"], "故障模板");

    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/templates"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(list.expect(StatusCode::OK).as_array().unwrap().len(), 1);

    // 用模板建卡
    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks/from-template"),
        Some(&owner_token),
        Some(&json!({ "templateId": template_id })),
    )
    .await;
    let created = created.expect(StatusCode::CREATED);
    assert_eq!(created["title"], "排查线上问题");
    assert!(created["descriptionMd"]
        .as_str()
        .unwrap()
        .contains("复现"));

    // 覆盖标题
    let custom = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks/from-template"),
        Some(&owner_token),
        Some(&json!({ "templateId": template_id, "title": "今晚线上故障" })),
    )
    .await;
    assert_eq!(
        custom.expect(StatusCode::CREATED)["title"],
        "今晚线上故障"
    );

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/templates/{template_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/templates"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(list
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn template_permissions_and_cross_board() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let (project, board, column) = setup(&app, &owner_token).await;
    add_member(&app, &owner_token, &project, viewer).await;

    // 设 viewer 只读
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(&owner_token),
        Some(&json!({ "userId": viewer, "role": "viewer" })),
    )
    .await
    .expect(StatusCode::CREATED);

    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/templates"),
        Some(&viewer_token),
        Some(&json!({ "name": "x", "title": "y", "descriptionMd": "" })),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    let template = create_template(&app, &owner_token, &board, "模板").await;
    let template_id = template["id"].as_str().unwrap().to_string();

    // 跨看板
    let second = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&owner_token),
        Some(&json!({ "name": "另一板" })),
    )
    .await
    .expect(StatusCode::CREATED);
    let second_board = second["id"].as_str().unwrap().to_string();
    let second_column = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{second_board}/columns"),
        Some(&owner_token),
        Some(&json!({ "name": "列" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{second_column}/tasks/from-template"),
        Some(&owner_token),
        Some(&json!({ "templateId": template_id })),
    )
    .await;
    assert_eq!(
        denied.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 空名称
    let bad = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/templates"),
        Some(&owner_token),
        Some(&json!({ "name": "  ", "title": "x", "descriptionMd": "" })),
    )
    .await;
    assert_eq!(
        bad.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    let _ = column;
}
