//! P0 看板层（项目 → 多看板）失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 建项目自动生成「默认看板」；
//! - `GET/POST /projects/{id}/boards`、`PATCH/DELETE /boards/{id}`；
//! - `GET/POST /boards/{id}/columns`、`GET /boards/{id}/tasks`；
//! - 旧接口 `/projects/{id}/columns` 仍可用，落在默认看板；
//! - 删除非空看板 422；非成员 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_project(app: &TestApp, token: &str, name: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": name, "description": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn boards(app: &TestApp, token: &str, project: &str) -> Value {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
}

async fn create_board(app: &TestApp, token: &str, project: &str, name: &str) -> Value {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(token),
        Some(&json!({ "name": name })),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

async fn create_column(app: &TestApp, token: &str, board: &str, name: &str) -> Value {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/columns"),
        Some(token),
        Some(&json!({ "name": name })),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

async fn create_task(app: &TestApp, token: &str, column: &str, title: &str) -> Value {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": title, "descriptionMd": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

#[tokio::test]
async fn project_gets_default_board_and_custom_boards_are_isolated() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token, "招新项目").await;

    let list = boards(&app, &token, &project).await;
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 1, "自动生成默认看板");
    assert_eq!(rows[0]["name"], "默认看板");
    let default_board = rows[0]["id"].as_str().unwrap().to_string();

    // 旧接口的列落在默认看板
    let legacy_column = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/columns"),
        Some(&token),
        Some(&json!({ "name": "待办" })),
    )
    .await;
    let legacy_column = legacy_column.expect(StatusCode::CREATED);
    assert_eq!(legacy_column["boardId"], default_board);
    let legacy_list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/columns"),
        Some(&token),
        None,
    )
    .await;
    let legacy_rows = legacy_list.expect(StatusCode::OK);
    assert_eq!(
        legacy_rows.as_array().unwrap().len(),
        4,
        "默认三列 + 新建一列"
    );

    // 第二个看板与列、任务隔离
    let second = create_board(&app, &token, &project, "迭代看板").await;
    let second_id = second["id"].as_str().unwrap().to_string();
    let column = create_column(&app, &token, &second_id, "进行中").await;
    let column_id = column["id"].as_str().unwrap().to_string();
    create_task(&app, &token, &column_id, "第二板的卡片").await;

    let tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{second_id}/tasks"),
        Some(&token),
        None,
    )
    .await;
    let tasks = tasks.expect(StatusCode::OK);
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["title"], "第二板的卡片");

    let default_tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{default_board}/tasks"),
        Some(&token),
        None,
    )
    .await;
    assert!(default_tasks
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());

    let all_boards = boards(&app, &token, &project).await;
    assert_eq!(all_boards.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn rename_and_delete_board_rules() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let project = create_project(&app, &token, "看板规则").await;
    let board = create_board(&app, &token, &project, "临时看板").await;
    let board_id = board["id"].as_str().unwrap().to_string();
    let column = create_column(&app, &token, &board_id, "列").await;
    let column_id = column["id"].as_str().unwrap().to_string();

    let renamed = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/boards/{board_id}"),
        Some(&token),
        Some(&json!({ "name": "正式看板" })),
    )
    .await;
    assert_eq!(renamed.expect(StatusCode::OK)["name"], "正式看板");

    // 非空不能删
    let denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/boards/{board_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        denied.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 清空后可删
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/columns/{column_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/boards/{board_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let list = boards(&app, &token, &project).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn boards_require_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let project = create_project(&app, &owner_token, "权限项目").await;

    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&outsider_token),
        Some(&json!({ "name": "偷偷建" })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
