//! 项目编辑与删除集成测试。
//!
//! 覆盖 `PATCH /projects/{id}`、`DELETE /projects/{id}`：
//! 管理员/普通成员/非成员权限、名称校验、404、删除后的级联清理。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use task_service::entity::{
    column, comment, project, project_member, subtask, task, task_assignee, task_attachment,
};
use uuid::Uuid;

/// 创建项目并返回 ID。
async fn create_project(app: &TestApp, token: &str, name: &str) -> String {
    request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": name, "description": "初始描述" })),
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

#[tokio::test]
async fn update_project_admin_and_validation() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "旧项目").await;
    add_member(&app, &token, &project_id, member).await;

    // Owner 同时修改名称与描述
    let updated = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "name": "新项目", "description": "新描述" })),
    )
    .await;
    let updated = updated.expect(StatusCode::OK);
    assert_eq!(updated["id"], project_id);
    assert_eq!(updated["name"], "新项目");
    assert_eq!(updated["description"], "新描述");

    // 列表接口读到新值
    let projects = request(&app.app, "GET", "/api/v1/task/projects", Some(&token), None).await;
    let projects = projects.expect(StatusCode::OK);
    let found = projects
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == project_id)
        .expect("项目应在列表中");
    assert_eq!(found["name"], "新项目");
    assert_eq!(found["description"], "新描述");

    // 只传描述：名称保持不变；名称两端空白会被裁剪
    let only_description = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "description": "仅改描述" })),
    )
    .await;
    let only_description = only_description.expect(StatusCode::OK);
    assert_eq!(only_description["name"], "新项目");
    assert_eq!(only_description["description"], "仅改描述");
    let trimmed = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "name": "  带空格项目  " })),
    )
    .await;
    let trimmed = trimmed.expect(StatusCode::OK);
    assert_eq!(trimmed["name"], "带空格项目");

    // 空名称 / 超长名称 → 422，带字段错误
    let empty = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "name": "   " })),
    )
    .await;
    let empty = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(empty["code"], "TASK_VALIDATION");
    assert_eq!(empty["errors"][0]["field"], "name");
    let long = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "name": "a".repeat(65) })),
    )
    .await;
    long.expect(StatusCode::UNPROCESSABLE_ENTITY);

    // 空请求体字段：不修改且返回当前值
    let noop = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({})),
    )
    .await;
    let noop = noop.expect(StatusCode::OK);
    assert_eq!(noop["name"], "带空格项目");

    // 普通成员 → 403；非成员 → 403（不同错误码）
    let member_denied = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&member_token),
        Some(&json!({ "name": "成员改的" })),
    )
    .await;
    let member_denied = member_denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(member_denied["code"], "TASK_FORBIDDEN");
    let outsider_denied = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&outsider_token),
        Some(&json!({ "name": "外人改的" })),
    )
    .await;
    let outsider_denied = outsider_denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(outsider_denied["code"], "TASK_NOT_MEMBER");

    // 不存在 / 非法 UUID / 未登录
    let missing = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{}", Uuid::now_v7()),
        Some(&token),
        Some(&json!({ "name": "不存在" })),
    )
    .await;
    let missing = missing.expect(StatusCode::NOT_FOUND);
    assert_eq!(missing["code"], "TASK_PROJECT_NOT_FOUND");
    request(
        &app.app,
        "PATCH",
        "/api/v1/task/projects/not-a-uuid",
        Some(&token),
        Some(&json!({ "name": "非法" })),
    )
    .await
    .expect(StatusCode::BAD_REQUEST);
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        None,
        Some(&json!({ "name": "未登录" })),
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn delete_project_permissions_and_cascade() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let project_id = create_project(&app, &token, "待删除项目").await;
    let project_uuid: Uuid = project_id.parse().unwrap();
    add_member(&app, &token, &project_id, member).await;
    let column_id = first_column(&app, &token, &project_id).await;
    let column_uuid: Uuid = column_id.parse().unwrap();

    // 创建任务，并挂上子任务/评论/负责人/附件（附件直接写库，避免依赖存储目录）
    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/tasks"),
        Some(&token),
        Some(&json!({
            "columnId": column_id,
            "title": "会被级联删除",
            "assigneeId": member.to_string()
        })),
    )
    .await;
    let created = created.expect(StatusCode::CREATED);
    let task_id = created["id"].as_str().unwrap().to_string();
    let task_uuid: Uuid = task_id.parse().unwrap();
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/subtasks"),
        Some(&token),
        Some(&json!({ "title": "子任务" })),
    )
    .await
    .expect(StatusCode::CREATED);
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/tasks/{task_id}/comments"),
        Some(&token),
        Some(&json!({ "contentMd": "评论" })),
    )
    .await
    .expect(StatusCode::CREATED);
    let attachment_id = Uuid::now_v7();
    task_attachment::ActiveModel {
        id: Set(attachment_id),
        task_id: Set(task_uuid),
        name: Set("附件.txt".into()),
        size: Set(3),
        mime: Set("text/plain".into()),
        storage_key: Set(format!("{task_uuid}/{attachment_id}")),
        uploader_id: Set(owner),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(&app.state.db)
    .await
    .expect("插入附件");

    // 权限：普通成员 / 非成员 / 未登录 → 403/401，项目仍存在
    let member_denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&member_token),
        None,
    )
    .await;
    let member_denied = member_denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(member_denied["code"], "TASK_FORBIDDEN");
    let outsider_denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    let outsider_denied = outsider_denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(outsider_denied["code"], "TASK_NOT_MEMBER");
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{project_id}"),
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{}", Uuid::now_v7()),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);

    // 管理员删除 → 204
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    // 数据库级联：项目/成员/列/任务/负责人/子任务/评论/附件全部清理
    assert!(project::Entity::find_by_id(project_uuid)
        .one(&app.state.db)
        .await
        .expect("查询项目")
        .is_none());
    assert!(project_member::Entity::find()
        .filter(project_member::Column::ProjectId.eq(project_uuid))
        .all(&app.state.db)
        .await
        .expect("查询成员")
        .is_empty());
    assert!(column::Entity::find()
        .filter(column::Column::ProjectId.eq(project_uuid))
        .all(&app.state.db)
        .await
        .expect("查询列")
        .is_empty());
    assert!(task::Entity::find()
        .filter(task::Column::ProjectId.eq(project_uuid))
        .all(&app.state.db)
        .await
        .expect("查询任务")
        .is_empty());
    assert!(task_assignee::Entity::find()
        .filter(task_assignee::Column::TaskId.eq(task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询负责人")
        .is_empty());
    assert!(subtask::Entity::find()
        .filter(subtask::Column::TaskId.eq(task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询子任务")
        .is_empty());
    assert!(comment::Entity::find()
        .filter(comment::Column::TaskId.eq(task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询评论")
        .is_empty());
    assert!(task_attachment::Entity::find()
        .filter(task_attachment::Column::TaskId.eq(task_uuid))
        .all(&app.state.db)
        .await
        .expect("查询附件")
        .is_empty());

    // 删除后：列表为空、成员关系失效、重复删除/再次编辑 → 404
    let projects = request(&app.app, "GET", "/api/v1/task/projects", Some(&token), None).await;
    assert_eq!(projects.expect(StatusCode::OK).as_array().unwrap().len(), 0);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/projects/{project_id}/columns"),
        Some(&member_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/task/projects/{project_id}"),
        Some(&token),
        Some(&json!({ "name": "复活" })),
    )
    .await
    .expect(StatusCode::NOT_FOUND);

    // 项目删除后成员关系失效：用原列 ID 建任务 → 403
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/projects/{project_id}/tasks"),
        Some(&token),
        Some(&json!({ "columnId": column_uuid, "title": "孤儿任务" })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
