//! P2 阅读统计与最近浏览失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 打开文档（`GET /spaces/{id}/nodes/{nodeId}`）记录一次浏览（按用户去重，更新最近时间）；
//! - 节点详情返回 `viewCount`（浏览过的用户数）；
//! - `GET /recent?limit=`：我最近浏览的页面（含 spaceId/spaceName/title/updatedAt），按时间倒序；
//! - 已删除文档不出现在最近浏览里。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str, name: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": name, "type": "team" })),
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

async fn open_node(app: &TestApp, token: &str, space_id: &str, node: &str) -> serde_json::Value {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node}"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
}

#[tokio::test]
async fn view_count_tracks_distinct_users() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token, "统计库").await;
    add_member(&app, &space_id, member, "viewer").await;
    let page = create_page(&app, &owner_token, &space_id, "统计页面").await;

    let detail = open_node(&app, &owner_token, &space_id, &page).await;
    assert_eq!(detail["viewCount"], 1);

    // 同一用户重复打开不重复计数
    open_node(&app, &owner_token, &space_id, &page).await;
    let detail = open_node(&app, &member_token, &space_id, &page).await;
    assert_eq!(detail["viewCount"], 2);
}

#[tokio::test]
async fn recent_lists_my_views_only() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token, "最近库").await;
    add_member(&app, &space_id, member, "viewer").await;
    let first = create_page(&app, &owner_token, &space_id, "第一篇").await;
    let second = create_page(&app, &owner_token, &space_id, "第二篇").await;

    open_node(&app, &member_token, &space_id, &first).await;
    open_node(&app, &member_token, &space_id, &second).await;

    let recent = request(
        &app.app,
        "GET",
        "/api/v1/doc/recent?limit=10",
        Some(&member_token),
        None,
    )
    .await;
    let recent = recent.expect(StatusCode::OK);
    let rows = recent.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], second, "最近打开的在前");
    assert_eq!(rows[0]["spaceName"], "最近库");
    assert_eq!(rows[0]["title"], "第二篇");

    // owner 没打开过 → 空
    let owner_recent = request(
        &app.app,
        "GET",
        "/api/v1/doc/recent",
        Some(&owner_token),
        None,
    )
    .await;
    assert!(owner_recent
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn deleted_docs_drop_out_of_recent() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let space_id = create_space(&app, &owner_token, "删除库").await;
    let page = create_page(&app, &owner_token, &space_id, "待删").await;
    open_node(&app, &owner_token, &space_id, &page).await;

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let recent = request(
        &app.app,
        "GET",
        "/api/v1/doc/recent",
        Some(&owner_token),
        None,
    )
    .await;
    assert!(recent
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());
}
