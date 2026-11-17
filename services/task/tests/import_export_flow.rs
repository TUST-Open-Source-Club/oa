//! P2 看板导入导出失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /boards/{id}/export` 返回 `{format, version, board, columns, labels, tasks}`；
//! - `POST /projects/{id}/import` 接受上述格式（往返），也兼容 Planka 的
//!   `{board:{name}, lists:[{name,position}], cards:[{name,description,listName?,labelNames?}]}`；
//! - 导入创建新看板 + 列 + 标签 + 卡片；viewer 403。

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
        Some(&json!({ "name": "导入导出", "description": "" })),
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

async fn seed(app: &TestApp, token: &str, board: &str, column: &str) {
    let label = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/labels"),
        Some(token),
        Some(&json!({ "name": "招新", "color": "#3b82f6" })),
    )
    .await;
    let label_id = label.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": "发推文", "descriptionMd": "写文案" })),
    )
    .await;
    let task_id = task.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/labels/{label_id}"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn export_and_roundtrip_import() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (project, board, column) = setup(&app, &token).await;
    seed(&app, &token, &board, &column).await;

    let exported = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/export"),
        Some(&token),
        None,
    )
    .await;
    let exported = exported.expect(StatusCode::OK);
    assert_eq!(exported["format"], "club-oa.task/board");
    assert_eq!(exported["board"]["name"], "默认看板");
    assert_eq!(exported["columns"].as_array().unwrap().len(), 3);
    assert_eq!(exported["labels"][0]["name"], "招新");
    assert_eq!(exported["tasks"][0]["title"], "发推文");
    assert_eq!(exported["tasks"][0]["labels"][0], "招新");

    // 原样导入 → 新看板
    let imported = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/import"),
        Some(&token),
        Some(&exported),
    )
    .await;
    let imported = imported.expect(StatusCode::CREATED);
    let new_board = imported["boardId"].as_str().unwrap().to_string();
    assert_eq!(imported["tasks"], 1);

    let boards = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project}/boards"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(boards.expect(StatusCode::OK).as_array().unwrap().len(), 2);

    let tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{new_board}/tasks"),
        Some(&token),
        None,
    )
    .await;
    let tasks = tasks.expect(StatusCode::OK);
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["title"], "发推文");
    assert_eq!(tasks[0]["labels"][0]["name"], "招新");
}

#[tokio::test]
async fn import_planka_export_and_permissions() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let (project, board, _) = setup(&app, &owner_token).await;

    let planka = json!({
        "board": { "name": "Planka 看板" },
        "lists": [
            { "name": "待办", "position": 1 },
            { "name": "进行中", "position": 2 }
        ],
        "cards": [
            { "name": "写周报", "description": "本周总结", "listName": "进行中" },
            { "name": "订场地", "description": "" }
        ],
        "labels": [{ "name": "行政", "color": "#f97316" }]
    });
    let imported = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/import"),
        Some(&owner_token),
        Some(&planka),
    )
    .await;
    let imported = imported.expect(StatusCode::CREATED);
    assert_eq!(imported["tasks"], 2);
    let new_board = imported["boardId"].as_str().unwrap().to_string();

    let columns = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{new_board}/columns"),
        Some(&owner_token),
        None,
    )
    .await;
    let columns = columns.expect(StatusCode::OK);
    assert_eq!(columns.as_array().unwrap().len(), 2);
    let in_progress = columns
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "进行中")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{new_board}/tasks"),
        Some(&owner_token),
        None,
    )
    .await;
    let tasks = tasks.expect(StatusCode::OK);
    let weekly = tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["title"] == "写周报")
        .unwrap();
    assert_eq!(weekly["columnId"], in_progress);

    // viewer 不能导入（需要项目管理员）
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/members"),
        Some(&owner_token),
        Some(&json!({ "userIds": [viewer] })),
    )
    .await
    .expect(StatusCode::OK);
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project}/import"),
        Some(&viewer_token),
        Some(&Value::Object(Default::default())),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    let _ = board;
}
