//! 单任务读取/更新/删除、认领与状态流转集成测试。
//!
//! 覆盖 `GET|PATCH|DELETE /tasks/{id}`、`POST /tasks/{id}/claim`、
//! `POST /tasks/{id}/status`，包括成员/非成员权限、校验失败、404 与删除级联。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use task_service::entity::{comment, subtask, task, task_assignee};
use uuid::Uuid;

/// 创建项目并返回 ID。
async fn create_project(app: &TestApp, token: &str, name: &str) -> String {
    request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": name, "description": "任务管理测试" })),
    )
    .await
    .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 把用户加入项目。
async fn add_member(app: &TestApp, token: &str, project_id: &str, user_id: Uuid) {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/members"),
        Some(token),
        Some(&json!({ "userIds": [user_id] })),
    )
    .await
    .expect(StatusCode::OK);
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

/// 创建任务并返回响应 JSON。
async fn create_task(
    app: &TestApp,
    token: &str,
    project_id: &str,
    column_id: &str,
    body: serde_json::Value,
) -> serde_json::Value {
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

/// 断言 JSON 数组包含全部给定 ID。
fn assert_ids_contain(value: &serde_json::Value, expected: &[Uuid]) {
    let ids = value.as_array().expect("应为数组");
    for id in expected {
        assert!(
            ids.iter()
                .any(|item| item.as_str() == Some(&id.to_string())),
            "负责人列表缺少 {id}: {value}"
        );
    }
}

#[tokio::test]
async fn task_get_update_and_delete() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let peer = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let peer_token = issue_token(&app, peer);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "任务管理").await;
    let column_id = first_column(&app, &token, &project_id).await;
    add_member(&app, &token, &project_id, peer).await;

    let due = chrono::Utc::now() + chrono::Duration::days(2);
    let start = chrono::Utc::now();
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({
            "title": "初始任务",
            "descriptionMd": "初始描述",
            "assigneeId": peer.to_string(),
            "priority": "high",
            "dueAt": due.to_rfc3339(),
            "startAt": start.to_rfc3339()
        }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();

    // 详情：字段完整、负责人为 peer
    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        None,
    )
    .await;
    let detail = detail.expect(StatusCode::OK);
    assert_eq!(detail["id"], task_id);
    assert_eq!(detail["projectId"], project_id);
    assert_eq!(detail["columnId"], column_id);
    assert_eq!(detail["title"], "初始任务");
    assert_eq!(detail["status"], "active");
    assert_eq!(detail["assigneeIds"], json!([peer.to_string()]));

    // 非成员 / 不存在 / 未登录
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{}", Uuid::now_v7()),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/tasks/{task_id}"),
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    // 普通成员可更新标题/描述/优先级
    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({
            "title": "修改后标题",
            "descriptionMd": "修改后描述",
            "priority": "low"
        })),
    )
    .await;
    let updated = updated.expect(StatusCode::OK);
    assert_eq!(updated["title"], "修改后标题");
    assert_eq!(updated["descriptionMd"], "修改后描述");
    assert_eq!(updated["priority"], "low");

    // 覆盖负责人为单人
    let reassigned = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({ "assigneeIds": [owner] })),
    )
    .await;
    let reassigned = reassigned.expect(StatusCode::OK);
    assert_eq!(reassigned["assigneeIds"], json!([owner.to_string()]));
    assert_eq!(reassigned["assigneeId"], owner.to_string());

    // 清空时间字段
    let cleared = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({ "dueAt": null, "startAt": null })),
    )
    .await;
    let cleared = cleared.expect(StatusCode::OK);
    assert!(cleared["dueAt"].is_null());
    assert!(cleared["startAt"].is_null());

    // 空负责人数组 / 显式 null 都表示清空
    let no_assignees = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({ "assigneeIds": [] })),
    )
    .await;
    let no_assignees = no_assignees.expect(StatusCode::OK);
    assert_eq!(no_assignees["assigneeIds"], json!([]));
    assert!(no_assignees["assigneeId"].is_null());

    // 单人 assigneeId 副本字段更新
    let single = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({ "assigneeId": peer })),
    )
    .await;
    let single = single.expect(StatusCode::OK);
    assert_eq!(single["assigneeIds"], json!([peer.to_string()]));

    // 非法优先级 / 非成员修改 / 不存在 / 空请求体
    let bad_priority = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
        Some(&json!({ "priority": "super" })),
    )
    .await;
    let bad_priority = bad_priority.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bad_priority["code"], "TASK_VALIDATION");
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&outsider_token),
        Some(&json!({ "title": "越权" })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{}", Uuid::now_v7()),
        Some(&token),
        Some(&json!({ "title": "不存在" })),
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    let noop = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        Some(&json!({})),
    )
    .await;
    let noop = noop.expect(StatusCode::OK);
    assert_eq!(noop["title"], "修改后标题");

    // 普通成员即可删除任务 → 204，随后 404/重复删除 404
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&peer_token),
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
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{task_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);

    // 非成员不能删除他人任务
    let other = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "受保护任务" }),
    )
    .await;
    let other_id = other["id"].as_str().unwrap().to_string();
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{other_id}"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 删除任务时级联清理负责人/子任务/评论
    let with_children = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "带子数据任务", "assigneeId": owner.to_string() }),
    )
    .await;
    let child_task_id = with_children["id"].as_str().unwrap().to_string();
    let child_task_uuid: Uuid = child_task_id.parse().unwrap();
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{child_task_id}/subtasks"),
        Some(&token),
        Some(&json!({ "title": "子任务" })),
    )
    .await
    .expect(StatusCode::CREATED);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{child_task_id}/comments"),
        Some(&token),
        Some(&json!({ "contentMd": "评论" })),
    )
    .await
    .expect(StatusCode::CREATED);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/tasks/{child_task_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    assert!(task::Entity::find_by_id(child_task_uuid)
        .one(&app.state.db)
        .await
        .expect("查询任务")
        .is_none());
    assert!(task_assignee::Entity::find()
        .filter(task_assignee::Column::TaskId.eq(child_task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询负责人")
        .is_empty());
    assert!(subtask::Entity::find()
        .filter(subtask::Column::TaskId.eq(child_task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询子任务")
        .is_empty());
    assert!(comment::Entity::find()
        .filter(comment::Column::TaskId.eq(child_task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询评论")
        .is_empty());
}

#[tokio::test]
async fn claim_task_flow() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member_a = Uuid::now_v7();
    let member_b = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let token_a = issue_token(&app, member_a);
    let token_b = issue_token(&app, member_b);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "任务认领").await;
    let project_uuid: Uuid = project_id.parse().unwrap();
    let column_id = first_column(&app, &token, &project_id).await;
    let column_uuid: Uuid = column_id.parse().unwrap();
    add_member(&app, &token, &project_id, member_a).await;
    add_member(&app, &token, &project_id, member_b).await;

    // 未指派任务：成员认领后成为负责人
    let unassigned = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "待认领任务" }),
    )
    .await;
    let task_id = unassigned["id"].as_str().unwrap().to_string();
    let claimed = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/claim"),
        Some(&token_a),
        None,
    )
    .await;
    let claimed = claimed.expect(StatusCode::OK);
    assert_eq!(claimed["assigneeIds"], json!([member_a.to_string()]));
    assert_eq!(claimed["assigneeId"], member_a.to_string());

    // 重复认领幂等
    let again = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/claim"),
        Some(&token_a),
        None,
    )
    .await;
    let again = again.expect(StatusCode::OK);
    assert_eq!(again["assigneeIds"].as_array().unwrap().len(), 1);

    // 第二个成员认领 → 多负责人
    let second = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/claim"),
        Some(&token_b),
        None,
    )
    .await;
    let second = second.expect(StatusCode::OK);
    assert_eq!(second["assigneeIds"].as_array().unwrap().len(), 2);
    assert_ids_contain(&second["assigneeIds"], &[member_a, member_b]);

    // 非成员 / 不存在 / 未登录
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/claim"),
        Some(&outsider_token),
        None,
    )
    .await;
    let denied = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(denied["code"], "TASK_NOT_MEMBER");
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{}/claim", Uuid::now_v7()),
        Some(&token_a),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/claim"),
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    // 已被指派给 owner 的任务：认领追加而非替换
    let assigned = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "已指派任务", "assigneeId": owner.to_string() }),
    )
    .await;
    let assigned_id = assigned["id"].as_str().unwrap().to_string();
    let claim_assigned = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{assigned_id}/claim"),
        Some(&token_a),
        None,
    )
    .await;
    let claim_assigned = claim_assigned.expect(StatusCode::OK);
    assert_eq!(claim_assigned["assigneeIds"].as_array().unwrap().len(), 2);
    assert_ids_contain(&claim_assigned["assigneeIds"], &[owner, member_a]);

    // 历史数据：tasks.assignee_id 有值但 task_assignees 为空，认领时保留原负责人
    let legacy_id = Uuid::now_v7();
    task::ActiveModel {
        id: Set(legacy_id),
        project_id: Set(project_uuid),
        column_id: Set(column_uuid),
        title: Set("历史任务".into()),
        description_md: Set(String::new()),
        assignee_id: Set(Some(owner)),
        start_at: Set(None),
        priority: Set("normal".into()),
        due_at: Set(None),
        position: Set(99),
        created_by: Set(owner),
        status: Set("active".into()),
        completed_at: Set(None),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(&app.state.db)
    .await
    .expect("插入历史任务");
    let legacy_claimed = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{legacy_id}/claim"),
        Some(&token_a),
        None,
    )
    .await;
    let legacy_claimed = legacy_claimed.expect(StatusCode::OK);
    assert_eq!(legacy_claimed["assigneeIds"].as_array().unwrap().len(), 2);
    assert_ids_contain(&legacy_claimed["assigneeIds"], &[owner, member_a]);
}

#[tokio::test]
async fn task_status_transitions() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "任务状态").await;
    let column_id = first_column(&app, &token, &project_id).await;
    let created = create_task(
        &app,
        &token,
        &project_id,
        &column_id,
        json!({ "title": "状态任务" }),
    )
    .await;
    let task_id = created["id"].as_str().unwrap().to_string();

    // 非法状态 → 422
    let bad = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        Some(&token),
        Some(&json!({ "status": "paused" })),
    )
    .await;
    let bad = bad.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bad["code"], "TASK_VALIDATION");

    // done → 记录 completedAt
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

    // terminated → 清空 completedAt
    let terminated = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        Some(&token),
        Some(&json!({ "status": "terminated" })),
    )
    .await;
    let terminated = terminated.expect(StatusCode::OK);
    assert_eq!(terminated["status"], "terminated");
    assert!(terminated["completedAt"].is_null());

    // active → 恢复
    let active = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        Some(&token),
        Some(&json!({ "status": "active" })),
    )
    .await;
    let active = active.expect(StatusCode::OK);
    assert_eq!(active["status"], "active");
    assert!(active["completedAt"].is_null());

    // 权限 / 不存在 / 未登录
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        Some(&outsider_token),
        Some(&json!({ "status": "done" })),
    )
    .await;
    let denied = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(denied["code"], "TASK_NOT_MEMBER");
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{}/status", Uuid::now_v7()),
        Some(&token),
        Some(&json!({ "status": "done" })),
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/status"),
        None,
        Some(&json!({ "status": "done" })),
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);
}
