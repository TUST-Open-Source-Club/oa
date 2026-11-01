//! P2 列 WIP 限制失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /columns/{id}` `{ name?, wipLimit? }`（editor 可改，null 清除限制）；
//! - 列 DTO 返回 `wipLimit`；
//! - 移到已达上限的列 → 422 `TASK_WIP_LIMIT`；同列移动/在未满列新建不受影响；
//! - 新建/建卡时把限制视为软限制：`POST /columns/{id}/tasks` 超限也拒绝。

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
        Some(&json!({ "name": "WIP 项目", "description": "" })),
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
    let columns = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/columns"),
        Some(token),
        None,
    )
    .await;
    let columns = columns.expect(StatusCode::OK);
    let first = columns[0]["id"].as_str().unwrap().to_string();
    let second = columns[1]["id"].as_str().unwrap().to_string();
    (board, first, second)
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

async fn set_wip(app: &TestApp, token: &str, column: &str, limit: Value) -> TestResponse {
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/columns/{column}"),
        Some(token),
        Some(&json!({ "wipLimit": limit })),
    )
    .await
}

#[tokio::test]
async fn wip_limit_blocks_moves_and_creates() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (_, first, second) = setup(&app, &token).await;

    let updated = set_wip(&app, &token, &second, json!(1)).await;
    assert_eq!(updated.expect(StatusCode::OK)["wipLimit"], 1);

    let occupied = create_task(&app, &token, &second, "占位").await;
    let waiting = create_task(&app, &token, &first, "等待").await;

    // 移入已满列
    let blocked = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{waiting}/move"),
        Some(&token),
        Some(&json!({ "columnId": second })),
    )
    .await;
    let blocked = blocked.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(blocked["code"], "TASK_WIP_LIMIT");

    // 超限新建
    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{second}/tasks"),
        Some(&token),
        Some(&json!({ "title": "又来一个" })),
    )
    .await;
    assert_eq!(
        created.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_WIP_LIMIT"
    );

    // 同列移动不拦
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{occupied}/move"),
        Some(&token),
        Some(&json!({ "columnId": second })),
    )
    .await
    .expect(StatusCode::OK);

    // 清除限制后可以移入
    set_wip(&app, &token, &second, json!(null))
        .await
        .expect(StatusCode::OK);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{waiting}/move"),
        Some(&token),
        Some(&json!({ "columnId": second })),
    )
    .await
    .expect(StatusCode::OK);
}
