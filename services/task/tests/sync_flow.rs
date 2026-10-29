//! P1 看板增量同步失败用例（TDD 红阶段）。
//!
//! 约定：`GET /boards/{id}/sync?since=<rfc3339>` 返回该时间之后变更的任务
//! `{ serverTime, tasks }`，用于轮询增量刷新。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

async fn setup(app: &TestApp, token: &str) -> (String, String, String) {
    let project = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "同步项目", "description": "" })),
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

#[tokio::test]
async fn sync_returns_only_changed_tasks() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let (_, board, column) = setup(&app, &token).await;

    let task = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(&token),
        Some(&json!({ "title": "初始标题" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 用「很早」的时间点取全量
    let early = "2020-01-01T00:00:00Z";
    let first = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/sync?since={early}"),
        Some(&token),
        None,
    )
    .await;
    let first = first.expect(StatusCode::OK);
    assert_eq!(first["tasks"].as_array().unwrap().len(), 1);
    assert!(first["serverTime"].as_str().is_some());

    // 以当前 serverTime 为游标，先拿到空结果
    let cursor = first["serverTime"].as_str().unwrap().to_string();
    let idle = request(
        &app.app,
        "GET",
        &format!(
            "/api/v1/task/boards/{board}/sync?since={}",
            urlencoding(&cursor)
        ),
        Some(&token),
        None,
    )
    .await;
    assert!(idle.expect(StatusCode::OK)["tasks"]
        .as_array()
        .unwrap()
        .is_empty());

    // 更新标题后，增量能拿到
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task}"),
        Some(&token),
        Some(&json!({ "title": "新标题" })),
    )
    .await
    .expect(StatusCode::OK);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let changed = request(
        &app.app,
        "GET",
        &format!(
            "/api/v1/task/boards/{board}/sync?since={}",
            urlencoding(&cursor)
        ),
        Some(&token),
        None,
    )
    .await;
    let changed = changed.expect(StatusCode::OK);
    let tasks = changed["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "新标题");

    // 归档后增量里不再包含（按未终止/未归档过滤）
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/archive"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let after_archive = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/sync?since={early}"),
        Some(&token),
        None,
    )
    .await;
    assert!(after_archive.expect(StatusCode::OK)["tasks"]
        .as_array()
        .unwrap()
        .is_empty());
}

/// 简单百分号编码（避免引入额外依赖）。
fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b':') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
