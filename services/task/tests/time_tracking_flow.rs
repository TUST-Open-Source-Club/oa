//! P2 工时与燃尽失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /tasks/{id}` 支持 `estimatedMinutes`（null 清除，1~14400）；
//! - `POST/GET /tasks/{id}/time-entries`、`DELETE /time-entries/{id}`；
//! - 任务 DTO 带 `estimatedMinutes` 与 `spentMinutes`；
//! - `GET /boards/{id}/burndown?days=7` 返回每日剩余预估分钟数；
//! - 0 分钟 422；viewer 403。

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
        Some(&json!({ "name": "工时项目", "description": "" })),
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

async fn detail(app: &TestApp, token: &str, task: &str) -> Value {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
}

#[tokio::test]
async fn estimate_and_time_entries() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (_, _, column) = setup(&app, &token).await;
    let task = create_task(&app, &token, &column, "写方案").await;

    // 预估
    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&token),
        Some(&json!({ "estimatedMinutes": 120 })),
    )
    .await;
    assert_eq!(updated.expect(StatusCode::OK)["estimatedMinutes"], 120);

    // 两条工时
    for (minutes, note) in [(60, "调研"), (30, "写初稿")] {
        request(
            &app.app,
            "POST",
            &format!("/api/v1/task/tasks/{task}/time-entries"),
            Some(&token),
            Some(&json!({ "minutes": minutes, "note": note })),
        )
        .await
        .expect(StatusCode::CREATED);
    }
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task}/time-entries"),
        Some(&token),
        None,
    )
    .await;
    let list = list.expect(StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2);
    assert_eq!(detail(&app, &token, &task).await["spentMinutes"], 90);

    // 删除 60 分钟那条
    let entry_id = list
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["minutes"] == 60)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/time-entries/{entry_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    assert_eq!(detail(&app, &token, &task).await["spentMinutes"], 30);

    // 清除预估
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&token),
        Some(&json!({ "estimatedMinutes": null })),
    )
    .await
    .expect(StatusCode::OK);
    assert!(detail(&app, &token, &task).await["estimatedMinutes"].is_null());

    // 非法分钟数
    let bad = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/time-entries"),
        Some(&token),
        Some(&json!({ "minutes": 0 })),
    )
    .await;
    assert_eq!(
        bad.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );
}

#[tokio::test]
async fn burndown_and_viewer_permissions() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let (project, board, column) = setup(&app, &owner_token).await;

    let first = create_task(&app, &owner_token, &column, "任务一").await;
    let second = create_task(&app, &owner_token, &column, "任务二").await;
    for (task, minutes) in [(&first, 60), (&second, 120)] {
        request(
            &app.app,
            "PATCH",
            &format!("/api/v1/task/tasks/{task}"),
            Some(&owner_token),
            Some(&json!({ "estimatedMinutes": minutes })),
        )
        .await
        .expect(StatusCode::OK);
    }
    // 完成第一个
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{first}/status"),
        Some(&owner_token),
        Some(&json!({ "status": "done" })),
    )
    .await
    .expect(StatusCode::OK);

    let burndown = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/burndown?days=7"),
        Some(&owner_token),
        None,
    )
    .await;
    let burndown = burndown.expect(StatusCode::OK);
    let days = burndown["days"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    let last = days.last().unwrap();
    assert_eq!(last["remainingMinutes"], 120, "完成的任务不再计入剩余");

    // viewer 不能记工时
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
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{second}/time-entries"),
        Some(&viewer_token),
        Some(&json!({ "minutes": 10 })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
