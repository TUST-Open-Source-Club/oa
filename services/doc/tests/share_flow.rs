//! P0 文档可见性与分享链接失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /spaces/{id}/nodes/{nodeId}` 支持 `visibility`：`space`（默认，空间成员可读）/ `private`（仅编辑者）；
//! - `POST /spaces/{id}/nodes/{nodeId}/share`：生成/更新分享链接（仅编辑者），
//!   body `{ password?, expiresAt?, permission? }`，返回 `{ token, url, hasPassword, expiresAt, permission }`；
//! - `DELETE .../share`：关闭分享；
//! - `GET /api/v1/doc/public/{token}?password=`：免登录只读访问，密码错误 401、
//!   过期 410、不存在 404；
//! - `viewer` 角色看不到/打不开 private 文档，编辑者不受影响。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "分享库", "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_page(app: &TestApp, token: &str, space_id: &str, title: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&json!({ "kind": "page", "title": title })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn write_content(app: &TestApp, token: &str, space_id: &str, node_id: &str, md: &str) {
    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node_id}/content"),
        Some(token),
        Some(&json!({ "contentMd": md, "baseVersion": 1 })),
    )
    .await
    .expect(StatusCode::OK);
}

/// 把用户加进空间（viewer 等角色没有邀请接口，测试里直接写库）。
async fn add_member(app: &TestApp, space_id: &str, user: Uuid, role: &str) {
    use doc_service::entity::space_member;
    space_member::ActiveModel {
        space_id: Set(space_id.parse().unwrap()),
        user_id: Set(user),
        role: Set(role.to_string()),
        joined_at: Set(app.state.now().fixed_offset()),
    }
    .insert(&app.state.db)
    .await
    .expect("加成员");
}

#[tokio::test]
async fn share_link_serves_public_readonly_copy() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let space_id = create_space(&app, &token).await;
    let page_id = create_page(&app, &token, &space_id, "对外文档").await;
    write_content(&app, &token, &space_id, &page_id, "# 对外内容\n\n正文").await;

    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}/share"),
        Some(&token),
        Some(&json!({ "permission": "read" })),
    )
    .await;
    let body = created.expect(StatusCode::OK);
    let share_token = body["token"].as_str().unwrap().to_string();
    assert!(body["url"].as_str().unwrap().contains(&share_token));
    assert_eq!(body["hasPassword"], false);

    // 免登录读取
    let public = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}"),
        None,
        None,
    )
    .await;
    let body = public.expect(StatusCode::OK);
    assert_eq!(body["title"], "对外文档");
    assert!(body["contentMd"].as_str().unwrap().contains("对外内容"));

    // 关闭分享后 404
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}/share"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}"),
        None,
        None,
    )
    .await
    .expect(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_password_and_expiry() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let space_id = create_space(&app, &token).await;
    let page_id = create_page(&app, &token, &space_id, "加密文档").await;
    write_content(&app, &token, &space_id, &page_id, "机密正文").await;

    let expires = app.state.now() + chrono::Duration::hours(1);
    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}/share"),
        Some(&token),
        Some(&json!({
            "password": "letmein",
            "expiresAt": expires.to_rfc3339(),
            "permission": "read",
        })),
    )
    .await;
    let body = created.expect(StatusCode::OK);
    let share_token = body["token"].as_str().unwrap().to_string();
    assert_eq!(body["hasPassword"], true);

    // 未带密码 → 401
    let denied = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}"),
        None,
        None,
    )
    .await;
    let body = denied.expect(StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "DOC_SHARE_PASSWORD_REQUIRED");

    // 密码错误 → 401
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}?password=wrong"),
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    // 正确密码 → 200
    let ok = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}?password=letmein"),
        None,
        None,
    )
    .await;
    let body = ok.expect(StatusCode::OK);
    assert!(body["contentMd"].as_str().unwrap().contains("机密正文"));
}

#[tokio::test]
async fn expired_share_returns_gone() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let token = issue_token(&app, owner);
    let space_id = create_space(&app, &token).await;
    let page_id = create_page(&app, &token, &space_id, "过期文档").await;
    write_content(&app, &token, &space_id, &page_id, "正文").await;

    let expires = app.state.now() - chrono::Duration::minutes(1);
    let created = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}/share"),
        Some(&token),
        Some(&json!({ "expiresAt": expires.to_rfc3339() })),
    )
    .await;
    let share_token = created.expect(StatusCode::OK)["token"]
        .as_str()
        .unwrap()
        .to_string();

    let gone = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/public/{share_token}"),
        None,
        None,
    )
    .await;
    let body = gone.expect(StatusCode::GONE);
    assert_eq!(body["code"], "DOC_SHARE_EXPIRED");
}

#[tokio::test]
async fn private_visibility_hides_from_viewers() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, viewer, "viewer").await;

    let private_page = create_page(&app, &owner_token, &space_id, "私密草稿").await;
    let public_page = create_page(&app, &owner_token, &space_id, "公开页面").await;

    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{private_page}"),
        Some(&owner_token),
        Some(&json!({ "visibility": "private" })),
    )
    .await
    .expect(StatusCode::OK);

    // viewer 的树里看不到私密文档
    let tree = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/tree"),
        Some(&viewer_token),
        None,
    )
    .await;
    let tree = tree.expect(StatusCode::OK);
    let ids: Vec<&str> = tree
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&public_page.as_str()));
    assert!(!ids.contains(&private_page.as_str()));

    // viewer 直接打不开私密文档
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{private_page}"),
        Some(&viewer_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 编辑者不受影响
    let owner_view = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{private_page}"),
        Some(&owner_token),
        None,
    )
    .await;
    owner_view.expect(StatusCode::OK);
}

#[tokio::test]
async fn share_requires_editor_role() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, viewer, "viewer").await;
    let page_id = create_page(&app, &owner_token, &space_id, "文档").await;

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}/share"),
        Some(&viewer_token),
        Some(&json!({})),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}

/// 让 `Value` 导入不为未使用（公共读取返回里用到）。
#[allow(dead_code)]
fn _unused(_: Value) {}
