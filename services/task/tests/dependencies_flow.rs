//! P2 卡片依赖失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST/DELETE /tasks/{id}/dependencies/{blocked_by_id}`；
//! - 任务 DTO 返回 `blockedBy: [{ id, title, status }]`；
//! - 只能依赖同看板的任务；不能依赖自己；不能形成环（422 `TASK_DEPENDENCY_CYCLE`）；
//! - viewer 403。

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
        Some(&json!({ "name": "依赖项目", "description": "" })),
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

async fn add_dependency(app: &TestApp, token: &str, task: &str, blocker: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/dependencies/{blocker}"),
        Some(token),
        None,
    )
    .await
}

async fn blocked_by(app: &TestApp, token: &str, task: &str) -> Value {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)["blockedBy"]
        .clone()
}

#[tokio::test]
async fn add_and_remove_dependency() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (_, _, column) = setup(&app, &token).await;
    let first = create_task(&app, &token, &column, "前置任务").await;
    let second = create_task(&app, &token, &column, "后续任务").await;

    add_dependency(&app, &token, &second, &first)
        .await
        .expect(StatusCode::NO_CONTENT);

    let blockers = blocked_by(&app, &token, &second).await;
    assert_eq!(blockers.as_array().unwrap().len(), 1);
    assert_eq!(blockers[0]["id"], first);
    assert_eq!(blockers[0]["title"], "前置任务");
    assert_eq!(blockers[0]["status"], "active");

    // 重复添加幂等
    add_dependency(&app, &token, &second, &first)
        .await
        .expect(StatusCode::NO_CONTENT);
    assert_eq!(
        blocked_by(&app, &token, &second)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{second}/dependencies/{first}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    assert!(blocked_by(&app, &token, &second).await.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn dependency_validation() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let (project, board, column) = setup(&app, &owner_token).await;
    let first = create_task(&app, &owner_token, &column, "A").await;
    let second = create_task(&app, &owner_token, &column, "B").await;

    // 自己依赖自己
    add_dependency(&app, &owner_token, &first, &first)
        .await
        .expect(StatusCode::UNPROCESSABLE_ENTITY);

    // 环：A→B 后 B→A
    add_dependency(&app, &owner_token, &first, &second)
        .await
        .expect(StatusCode::NO_CONTENT);
    let cycle = add_dependency(&app, &owner_token, &second, &first).await;
    assert_eq!(
        cycle.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_DEPENDENCY_CYCLE"
    );

    // 跨看板
    let project2 = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(&owner_token),
        Some(&json!({ "name": "另一个项目", "description": "" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let board2 = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project2}/boards"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let column2 = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board2}/columns"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let foreign = create_task(&app, &owner_token, &column2, "别处任务").await;
    let cross = add_dependency(&app, &owner_token, &second, &foreign).await;
    assert_eq!(
        cross.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // viewer 不能操作
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
    add_dependency(&app, &viewer_token, &second, &first)
        .await
        .expect(StatusCode::FORBIDDEN);
}
