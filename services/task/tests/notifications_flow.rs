//! P1 任务通知增强失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 评论请求支持 `mentions: [userId]`，通知接收人 = 负责人 + 被 @ 的人 − 评论者；
//! - `reminders::remind_due_tasks` 扫描 24h 内到期且未提醒的任务，写 `task.due_soon` 事件，
//!   并记录 `due_reminded_at`，重复调用不会重复提醒。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn setup(app: &TestApp, owner: Uuid, token: &str) -> (String, String, String) {
    let project = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "通知项目", "description": "" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = owner;
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

fn targets(event: &Value) -> Vec<String> {
    event["targetUsers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

async fn event_of_type(app: &TestApp, event_type: &str) -> Value {
    let rows = club_bus::outbox::fetch_due(&app.state.db, 200, app.state.now())
        .await
        .expect("outbox");
    rows.into_iter()
        .find(|row| row.event_type == event_type)
        .unwrap_or_else(|| panic!("没有 {event_type} 事件"))
        .payload
}

#[tokio::test]
async fn comment_mentions_join_notification_targets() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let assignee = Uuid::now_v7();
    let mentioned = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let assignee_token = issue_token(&app, assignee);
    let (project, _, column) = setup(&app, owner, &owner_token).await;
    add_member(&app, &owner_token, &project, assignee).await;
    add_member(&app, &owner_token, &project, mentioned).await;

    let task = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(&owner_token),
        Some(&json!({ "title": "评审任务", "assigneeIds": [assignee] })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let comment = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task}/comments"),
        Some(&assignee_token),
        Some(&json!({ "contentMd": "@owner 看下这里", "mentions": [mentioned] })),
    )
    .await;
    comment.expect(StatusCode::CREATED);

    let event = event_of_type(&app, "task.comment.added").await;
    let mut expected = vec![mentioned.to_string()];
    // 评论者是负责人，负责人被排除
    assert_eq!(targets(&event), expected, "被 @ 的人应收到提醒");

    // 再加一个负责人时，负责人也在接收人里
    expected.sort();
}

#[tokio::test]
async fn due_reminder_is_sent_once() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let assignee = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let (project, board, column) = setup(&app, owner, &owner_token).await;
    add_member(&app, &owner_token, &project, assignee).await;

    let due = (chrono::Utc::now() + chrono::Duration::hours(12)).to_rfc3339();
    let task = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(&owner_token),
        Some(&json!({ "title": "快到期的任务", "assigneeIds": [assignee], "dueAt": due })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let first = task_service::reminders::remind_due_tasks(&app.state)
        .await
        .expect("提醒");
    assert_eq!(first, 1, "应提醒 1 个任务");

    let event = event_of_type(&app, "task.due_soon").await;
    assert!(event["title"].as_str().unwrap().contains("快到期的任务"));
    assert_eq!(targets(&event), vec![assignee.to_string()]);

    let second = task_service::reminders::remind_due_tasks(&app.state)
        .await
        .expect("再次提醒");
    assert_eq!(second, 0, "同一天不重复提醒");

    let _ = (board, task);
}
