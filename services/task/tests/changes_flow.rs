//! task 变更日志与撤销集成测试。
//!
//! 覆盖 `GET /tasks/{id}/changes` 与 `POST /changes/{id}/undo`：
//! 更新撤销、删除恢复、状态回滚，以及权限不足、记录不存在、不可撤销记录等错误分支。
//! 全部用例走真实 Postgres（每个用例独立 schema）。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// 创建项目并返回项目 ID。
async fn create_project(app: &TestApp, token: &str, name: &str) -> String {
    request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": name, "description": "变更日志测试" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 读取项目首个看板列 ID。
async fn first_column(app: &TestApp, token: &str, project_id: &str) -> String {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project_id}/columns"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 创建任务并返回响应 JSON（自动补 `columnId`）。
async fn create_task(
    app: &TestApp,
    token: &str,
    project_id: &str,
    column_id: &str,
    body: Value,
) -> Value {
    let mut payload = body;
    payload["columnId"] = json!(column_id);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/tasks"),
        Some(token),
        Some(&payload),
    )
    .await
    .expect(StatusCode::CREATED)
}

/// 读取任务变更记录列表。
async fn list_changes(app: &TestApp, token: &str, task_id: &str) -> Vec<Value> {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}/changes"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
    .as_array()
    .unwrap()
    .clone()
}

/// 撤销一条变更。
async fn undo_change(app: &TestApp, token: &str, change_id: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/changes/{change_id}/undo"),
        Some(token),
        None,
    )
    .await
}

#[tokio::test]
async fn update_change_is_recorded_and_undone() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let peer = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let peer_token = issue_token(&app, peer);
    let project_id = create_project(&app, &token, "变更撤销").await;
    let column_id = first_column(&app, &token, &project_id).await;
    // peer 需要是项目成员，撤销后才能读取任务。
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/members"),
        Some(&token),
        Some(&json!({ "userIds": [peer] })),
    )
    .await
    .expect(StatusCode::OK);

    let due = chrono::Utc::now() + chrono::Duration::days(3);
    let start = chrono::Utc::now() + chrono::Duration::hours(2);
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({
            "title": "原始标题",
            "descriptionMd": "原始描述",
            "assigneeId": peer.to_string(),
            "priority": "high",
            "dueAt": due.to_rfc3339(),
            "startAt": start.to_rfc3339()
        }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();

    // 更新：改标题/优先级、清空时间、替换负责人
    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        Some(&json!({
            "title": "新标题",
            "descriptionMd": "新描述",
            "priority": "urgent",
            "dueAt": null,
            "startAt": null,
            "assigneeIds": [owner]
        })),
    )
    .await;
    let updated = updated.expect(StatusCode::OK);
    assert_eq!(updated["title"], "新标题");
    assert!(updated["dueAt"].is_null());
    assert!(updated["startAt"].is_null());

    // 变更日志：更新前/后快照与操作者
    let changes = list_changes(&app, &token, &task_id).await;
    assert_eq!(changes.len(), 1);
    let entry = &changes[0];
    assert_eq!(entry["entity"], "task");
    assert_eq!(entry["entity_id"], task_id);
    assert_eq!(entry["action"], "update");
    assert_eq!(entry["actor_id"], owner.to_string());
    assert_eq!(entry["before"]["title"], "原始标题");
    assert_eq!(entry["before"]["priority"], "high");
    assert_eq!(entry["before"]["assigneeIds"], json!([peer.to_string()]));
    assert_eq!(entry["after"]["title"], "新标题");
    assert_eq!(entry["after"]["priority"], "urgent");
    let change_id = entry["id"].as_str().unwrap().to_string();

    // peer 是成员，可以撤销（撤销不要求管理员）
    let undone = undo_change(&app, &peer_token, &change_id).await;
    let undone = undone.expect(StatusCode::OK);
    assert_eq!(undone["id"], task_id);
    assert_eq!(undone["title"], "原始标题");
    assert_eq!(undone["descriptionMd"], "原始描述");
    assert_eq!(undone["priority"], "high");
    assert_eq!(undone["assigneeIds"], json!([peer.to_string()]));
    let restored_due = chrono::DateTime::parse_from_rfc3339(undone["dueAt"].as_str().unwrap())
        .unwrap()
        .to_utc();
    assert_eq!(restored_due, due);
    let restored_start = chrono::DateTime::parse_from_rfc3339(undone["startAt"].as_str().unwrap())
        .unwrap()
        .to_utc();
    assert_eq!(restored_start, start);

    // 撤销本身也写入一条变更记录，且持久化生效
    let persisted = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        None,
    )
    .await;
    let persisted = persisted.expect(StatusCode::OK);
    assert_eq!(persisted["title"], "原始标题");
    assert_eq!(persisted["priority"], "high");
    let changes = list_changes(&app, &token, &task_id).await;
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["action"], "undo");
    assert_eq!(changes[0]["actor_id"], peer.to_string());
    assert_eq!(changes[1]["action"], "update");
}

#[tokio::test]
async fn delete_change_restores_task() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "删除恢复").await;
    let column_id = first_column(&app, &token, &project_id).await;
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/members"),
        Some(&token),
        Some(&json!({ "userIds": [member] })),
    )
    .await
    .expect(StatusCode::OK);

    let due = chrono::Utc::now() + chrono::Duration::days(1);
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({
            "title": "可恢复任务",
            "descriptionMd": "删除后按快照重建",
            "assigneeId": member.to_string(),
            "priority": "urgent",
            "dueAt": due.to_rfc3339()
        }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();
    let task_uuid: Uuid = task_id.parse().unwrap();

    // 删除任务：204，之后 404
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);

    // 任务已删除，只能通过审计接口拿到变更记录
    let entries = club_bus::audit::list_for(&app.state.db, "task", &task_id, 50)
        .await
        .expect("读取变更日志");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, "delete");
    assert_eq!(entries[0].entity_id, task_id);
    assert_eq!(entries[0].actor_id, Some(owner));
    assert!(entries[0].before.is_some());
    assert!(entries[0].after.is_none());
    let change_id = entries[0].id.to_string();

    // 非成员不能撤销删除
    let denied = undo_change(&app, &outsider_token, &change_id).await;
    let denied = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(denied["code"], "TASK_NOT_MEMBER");

    // 成员撤销删除 → 任务按原 ID/字段恢复
    let restored = undo_change(&app, &member_token, &change_id).await;
    let restored = restored.expect(StatusCode::OK);
    assert_eq!(restored["id"], task_id);
    assert_eq!(restored["title"], "可恢复任务");
    assert_eq!(restored["descriptionMd"], "删除后按快照重建");
    assert_eq!(restored["priority"], "urgent");
    assert_eq!(restored["status"], "active");
    assert_eq!(restored["assigneeIds"], json!([member.to_string()]));
    assert!(restored["completedAt"].is_null());
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(restored["dueAt"].as_str().unwrap())
            .unwrap()
            .to_utc(),
        due
    );

    // 恢复后：单任务与项目任务列表都能读到；变更日志包含删除与撤销两条
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&member_token),
        None,
    )
    .await
    .expect(StatusCode::OK);
    let tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project_id}/tasks"),
        Some(&token),
        None,
    )
    .await;
    let tasks = tasks.expect(StatusCode::OK);
    assert!(tasks
        .as_array()
        .unwrap()
        .iter()
        .any(|task| task["id"] == task_id));
    let changes = list_changes(&app, &token, &task_id).await;
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["action"], "undo");
    assert_eq!(changes[1]["action"], "delete");

    // 数据库确认任务确实存在（undo 记录中的 entity_id 指向同一任务）
    let row = task_service::repo::find_task(&app.state.db, task_uuid)
        .await
        .expect("查询任务");
    assert!(row.is_some());
}

#[tokio::test]
async fn status_change_is_recorded_and_undone() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let project_id = create_project(&app, &token, "状态撤销").await;
    let column_id = first_column(&app, &token, &project_id).await;
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "状态流转任务" }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();

    let done = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        Some(&token),
        Some(&json!({ "status": "done" })),
    )
    .await;
    let done = done.expect(StatusCode::OK);
    assert_eq!(done["status"], "done");
    assert!(!done["completedAt"].is_null());

    let changes = list_changes(&app, &token, &task_id).await;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["action"], "status");
    assert_eq!(changes[0]["before"]["status"], "active");
    assert_eq!(changes[0]["after"]["status"], "done");
    assert!(!changes[0]["after"]["completedAt"].is_null());
    let change_id = changes[0]["id"].as_str().unwrap().to_string();

    // 撤销完成 → 回到 active，完成时间清空
    let undone = undo_change(&app, &token, &change_id).await;
    let undone = undone.expect(StatusCode::OK);
    assert_eq!(undone["status"], "active");
    assert!(undone["completedAt"].is_null());
    let persisted = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        None,
    )
    .await;
    persisted.expect(StatusCode::OK);
    assert_eq!(list_changes(&app, &token, &task_id).await.len(), 2);
}

#[tokio::test]
async fn changes_permission_and_invalid_records() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "变更权限").await;
    let column_id = first_column(&app, &token, &project_id).await;
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "权限任务" }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        Some(&json!({ "title": "更新一次" })),
    )
    .await
    .expect(StatusCode::OK);
    let changes = list_changes(&app, &token, &task_id).await;
    let change_id = changes[0]["id"].as_str().unwrap().to_string();

    // 非成员读取/撤销 → 403
    let denied = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}/changes"),
        Some(&outsider_token),
        None,
    )
    .await;
    let denied = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(denied["code"], "TASK_NOT_MEMBER");
    undo_change(&app, &outsider_token, &change_id)
        .await
        .expect(StatusCode::FORBIDDEN);

    // 未登录 → 401
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}/changes"),
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    // 不存在的任务 / 变更记录 / 非法 UUID
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{}/changes", Uuid::now_v7()),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    let missing = undo_change(&app, &token, &Uuid::now_v7().to_string()).await;
    let missing = missing.expect(StatusCode::NOT_FOUND);
    assert_eq!(missing["code"], "TASK_CHANGE_NOT_FOUND");
    request(
        &app.app,
        "POST",
        "/api/v1/task/changes/not-a-uuid/undo",
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::BAD_REQUEST);

    // 非 task 实体的记录 → 422
    let project_change_id = club_bus::audit::record(
        &app.state.db,
        "project",
        &project_id,
        "update",
        Some(json!({ "name": "旧名" })),
        Some(json!({ "name": "新名" })),
        Some(owner),
        chrono::Utc::now(),
    )
    .await
    .expect("写入变更记录");
    let unsupported = undo_change(&app, &token, &project_change_id.to_string()).await;
    let unsupported = unsupported.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(unsupported["code"], "TASK_UNDO_UNSUPPORTED");

    // before 为空的记录不可撤销 → 422
    let no_before_id = club_bus::audit::record(
        &app.state.db,
        "task",
        &task_id,
        "update",
        None,
        None,
        Some(owner),
        chrono::Utc::now(),
    )
    .await
    .expect("写入变更记录");
    let no_before = undo_change(&app, &token, &no_before_id.to_string()).await;
    let no_before = no_before.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(no_before["code"], "TASK_UNDO_UNSUPPORTED");

    // 删除类记录缺少项目快照 → 422
    let bad_delete_id = club_bus::audit::record(
        &app.state.db,
        "task",
        &task_id,
        "delete",
        Some(json!({ "id": task_id })),
        None,
        Some(owner),
        chrono::Utc::now(),
    )
    .await
    .expect("写入变更记录");
    let bad_delete = undo_change(&app, &token, &bad_delete_id.to_string()).await;
    let bad_delete = bad_delete.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bad_delete["code"], "TASK_UNDO_UNSUPPORTED");
}
