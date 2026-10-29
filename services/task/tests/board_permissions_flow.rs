//! P1 看板权限失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 默认继承项目角色：项目管理员 = 看板 admin，普通成员 = 看板 editor；
//! - `board_members` 可覆盖为 viewer（只读）或 editor；
//! - `GET/POST /boards/{id}/members`、`PATCH/DELETE /boards/{id}/members/{userId}`，
//!   仅看板 admin 可管理；
//! - viewer 不能建列/建卡/移动卡（403），但可以看；
//! - 非项目成员访问 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_project(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/task/projects",
        Some(token),
        Some(&json!({ "name": "看板权限", "description": "" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn first_board(app: &TestApp, token: &str, project: &str) -> String {
    request(
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
        .to_string()
}

async fn first_column(app: &TestApp, token: &str, board: &str) -> String {
    request(
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
        .to_string()
}

async fn add_project_member(app: &TestApp, token: &str, project: &str, user: Uuid) {
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

async fn set_board_role(
    app: &TestApp,
    token: &str,
    board: &str,
    user: Uuid,
    role: &str,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(token),
        Some(&json!({ "userId": user, "role": role })),
    )
    .await
}

async fn create_task(app: &TestApp, token: &str, column: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/task/columns/{column}/tasks"),
        Some(token),
        Some(&json!({ "title": "权限卡片" })),
    )
    .await
}

/// 找成员角色。
fn role_of(list: &Value, user: Uuid) -> String {
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["userId"] == user.to_string())
        .unwrap_or_else(|| panic!("找不到成员 {user}: {list}"))["role"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn default_roles_and_viewer_override() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let project = create_project(&app, &owner_token).await;
    add_project_member(&app, &owner_token, &project, member).await;
    let board = first_board(&app, &owner_token, &project).await;
    let column = first_column(&app, &owner_token, &board).await;

    // 默认继承：owner=admin，member=editor
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(&owner_token),
        None,
    )
    .await;
    let list = list.expect(StatusCode::OK);
    assert_eq!(role_of(&list, owner), "admin");
    assert_eq!(role_of(&list, member), "editor");

    // 成员默认可建卡
    create_task(&app, &member_token, &column)
        .await
        .expect(StatusCode::CREATED);

    // 设为 viewer 后不能再建卡/建列，但能读
    set_board_role(&app, &owner_token, &board, member, "viewer")
        .await
        .expect(StatusCode::CREATED);
    create_task(&app, &member_token, &column)
        .await
        .expect(StatusCode::FORBIDDEN);
    let new_column = request(
        &app.app,
        "POST",
        &format!("/api/v1/task/boards/{board}/columns"),
        Some(&member_token),
        Some(&json!({ "name": "新列" })),
    )
    .await;
    new_column.expect(StatusCode::FORBIDDEN);
    let tasks = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks"),
        Some(&member_token),
        None,
    )
    .await;
    tasks.expect(StatusCode::OK);

    // 恢复 editor
    set_board_role(&app, &owner_token, &board, member, "editor")
        .await
        .expect(StatusCode::CREATED);
    create_task(&app, &member_token, &column)
        .await
        .expect(StatusCode::CREATED);
}

#[tokio::test]
async fn only_board_admin_manages_members() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let project = create_project(&app, &owner_token).await;
    add_project_member(&app, &owner_token, &project, member).await;
    let board = first_board(&app, &owner_token, &project).await;

    // editor 不能管理成员
    set_board_role(&app, &member_token, &board, outsider, "viewer")
        .await
        .expect(StatusCode::FORBIDDEN);

    // 非项目成员不能看成员/看板
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/tasks"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 非法角色
    let bad = set_board_role(&app, &owner_token, &board, member, "owner").await;
    assert_eq!(
        bad.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "TASK_VALIDATION"
    );

    // 删除覆盖后回到继承
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/task/boards/{board}/members/{member}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/task/boards/{board}/members"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(role_of(&list.expect(StatusCode::OK), member), "editor");
}
