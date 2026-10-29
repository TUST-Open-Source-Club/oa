//! P1 空间权限细化失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 空间创建者为 `owner`（唯一），其余角色 `admin` / `editor` / `viewer`；
//! - `GET/POST /spaces/{id}/members`、`PATCH/DELETE .../members/{userId}`、
//!   `POST /spaces/{id}/transfer`；
//! - owner/admin 可邀请与移除成员；admin 不能授予/移除 admin，也不能动 owner；
//! - 转移空间：owner → 目标成员，原 owner 降为 admin；仅 owner 可转移；
//! - 被移除的成员立即失去访问；非成员访问成员列表 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "权限库", "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn members_raw(app: &TestApp, token: &str, space_id: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/members"),
        Some(token),
        None,
    )
    .await
}

async fn members(app: &TestApp, token: &str, space_id: &str) -> serde_json::Value {
    members_raw(app, token, space_id)
        .await
        .expect(StatusCode::OK)
}

async fn invite(
    app: &TestApp,
    token: &str,
    space_id: &str,
    user: Uuid,
    role: &str,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/members"),
        Some(token),
        Some(&json!({ "userId": user, "role": role })),
    )
    .await
}

async fn patch_role(
    app: &TestApp,
    token: &str,
    space_id: &str,
    user: Uuid,
    role: &str,
) -> TestResponse {
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/members/{user}"),
        Some(token),
        Some(&json!({ "role": role })),
    )
    .await
}

async fn create_page(app: &TestApp, token: &str, space_id: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&json!({ "kind": "page", "title": "页面" })),
    )
    .await
}

/// 取成员角色。
fn role_of(list: &serde_json::Value, user: Uuid) -> String {
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
async fn owner_role_and_invites() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let editor = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let editor_token = issue_token(&app, editor);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;

    let list = members(&app, &owner_token, &space_id).await;
    assert_eq!(role_of(&list, owner), "owner", "创建者是 owner");

    invite(&app, &owner_token, &space_id, editor, "editor")
        .await
        .expect(StatusCode::CREATED);
    invite(&app, &owner_token, &space_id, viewer, "viewer")
        .await
        .expect(StatusCode::CREATED);

    // 角色生效
    create_page(&app, &editor_token, &space_id)
        .await
        .expect(StatusCode::CREATED);
    create_page(&app, &viewer_token, &space_id)
        .await
        .expect(StatusCode::FORBIDDEN);

    // editor 不能邀请人
    invite(&app, &editor_token, &space_id, Uuid::now_v7(), "viewer")
        .await
        .expect(StatusCode::FORBIDDEN);

    // 非法角色
    let invalid = invite(&app, &owner_token, &space_id, Uuid::now_v7(), "boss").await;
    assert_eq!(
        invalid.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );
}

#[tokio::test]
async fn admin_limits_and_removal() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let admin = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let admin_token = issue_token(&app, admin);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    invite(&app, &owner_token, &space_id, admin, "admin")
        .await
        .expect(StatusCode::CREATED);
    invite(&app, &owner_token, &space_id, viewer, "viewer")
        .await
        .expect(StatusCode::CREATED);

    // admin 可邀请 viewer、可移除 viewer
    invite(&app, &admin_token, &space_id, Uuid::now_v7(), "viewer")
        .await
        .expect(StatusCode::CREATED);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/members/{viewer}"),
        Some(&admin_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    members_raw(&app, &viewer_token, &space_id)
        .await
        .expect(StatusCode::FORBIDDEN);

    // admin 不能授予 admin / 动 owner
    let denied = patch_role(&app, &admin_token, &space_id, Uuid::now_v7(), "admin").await;
    denied.expect(StatusCode::NOT_FOUND);
    let denied = patch_role(&app, &admin_token, &space_id, owner, "viewer").await;
    denied.expect(StatusCode::FORBIDDEN);
    let denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/members/{owner}"),
        Some(&admin_token),
        None,
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    // owner 也不能直接移除 owner（只能转移后再操作）
    let denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/members/{owner}"),
        Some(&owner_token),
        None,
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn transfer_ownership_swaps_roles() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let successor = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let successor_token = issue_token(&app, successor);
    let space_id = create_space(&app, &owner_token).await;
    invite(&app, &owner_token, &space_id, successor, "editor")
        .await
        .expect(StatusCode::CREATED);

    // 非 owner 不能转移
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/transfer"),
        Some(&successor_token),
        Some(&json!({ "userId": owner })),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/transfer"),
        Some(&owner_token),
        Some(&json!({ "userId": successor })),
    )
    .await
    .expect(StatusCode::OK);

    let list = members(&app, &successor_token, &space_id).await;
    assert_eq!(role_of(&list, successor), "owner");
    assert_eq!(role_of(&list, owner), "admin");

    // 原 owner 不能再转移/删除空间
    let denied = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/transfer"),
        Some(&owner_token),
        Some(&json!({ "userId": owner })),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);
}
