//! P1 文档关注与事件失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /spaces/{id}/nodes/{nodeId}/follow` 切换关注，返回 `{ following }`；
//! - `GET .../follow` 返回当前用户是否关注；
//! - 评论/正文更新写 outbox 事件（`doc.comment.created` / `doc.updated`），
//!   接收人 = 文档作者 + 关注者 − 操作者。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "关注库", "type": "team" })),
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

async fn toggle_follow(app: &TestApp, token: &str, space_id: &str, page: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/follow"),
        Some(token),
        None,
    )
    .await
}

/// 取出 outbox 里第一条指定类型的事件 payload。
async fn event_of_type(app: &TestApp, event_type: &str) -> serde_json::Value {
    let rows = club_bus::outbox::fetch_due(&app.state.db, 100, app.state.now())
        .await
        .expect("读取 outbox");
    for row in rows {
        if row.event_type == event_type {
            return row.payload;
        }
    }
    panic!("没有找到事件 {event_type}");
}

fn targets(event: &serde_json::Value) -> Vec<String> {
    event["targetUsers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn follow_toggle_and_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, member, "viewer").await;
    let page = create_page(&app, &owner_token, &space_id, "关注页面").await;

    let followed = toggle_follow(&app, &member_token, &space_id, &page).await;
    assert_eq!(followed.expect(StatusCode::OK)["following"], true);

    let status = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/follow"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status.expect(StatusCode::OK)["following"], true);

    let unfollowed = toggle_follow(&app, &member_token, &space_id, &page).await;
    assert_eq!(unfollowed.expect(StatusCode::OK)["following"], false);

    toggle_follow(&app, &outsider_token, &space_id, &page)
        .await
        .expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn comment_emits_event_to_author_and_followers() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, member, "editor").await;
    let page = create_page(&app, &owner_token, &space_id, "评审文档").await;
    toggle_follow(&app, &member_token, &space_id, &page)
        .await
        .expect(StatusCode::OK);

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments"),
        Some(&member_token),
        Some(&json!({ "body": "这里需要补充数据" })),
    )
    .await
    .expect(StatusCode::CREATED);

    let event = event_of_type(&app, "doc.comment.created").await;
    assert!(event["title"].as_str().unwrap().contains("评审文档"));
    assert_eq!(event["actorId"], member.to_string());
    // 作者收到；操作者（同时也是关注者）被排除
    assert_eq!(targets(&event), vec![owner.to_string()]);
    assert!(event["resource"]["url"].as_str().unwrap().contains("docs"));
}

#[tokio::test]
async fn content_update_notifies_followers_not_editor() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, member, "editor").await;
    let page = create_page(&app, &owner_token, &space_id, "更新文档").await;

    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/content"),
        Some(&member_token),
        Some(&json!({ "contentMd": "第一次更新", "baseVersion": 1 })),
    )
    .await
    .expect(StatusCode::OK);

    let event = event_of_type(&app, "doc.updated").await;
    assert!(event["title"].as_str().unwrap().contains("更新文档"));
    assert_eq!(event["actorId"], member.to_string());
    // 作者收到；操作者被排除
    assert_eq!(targets(&event), vec![owner.to_string()]);

    // 关注者也会收到后续更新
    toggle_follow(&app, &member_token, &space_id, &page)
        .await
        .expect(StatusCode::OK);
    let latest = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}"),
        Some(&owner_token),
        None,
    )
    .await;
    let version = latest.expect(StatusCode::OK)["version"].as_i64().unwrap();
    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/content"),
        Some(&owner_token),
        Some(&json!({ "contentMd": "第二次更新", "baseVersion": version })),
    )
    .await
    .expect(StatusCode::OK);

    let rows = club_bus::outbox::fetch_due(&app.state.db, 100, app.state.now())
        .await
        .expect("读取 outbox");
    let events: Vec<serde_json::Value> = rows
        .into_iter()
        .filter(|row| row.event_type == "doc.updated")
        .map(|row| row.payload)
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(targets(&events[1]), vec![member.to_string()]);
}

#[tokio::test]
async fn comment_mentions_add_recipients() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let mentioned = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, member, "editor").await;
    add_member(&app, &space_id, mentioned, "viewer").await;
    let page = create_page(&app, &owner_token, &space_id, "提及文档").await;

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments"),
        Some(&member_token),
        Some(&json!({ "body": "@陈哲 看下这里", "mentions": [mentioned] })),
    )
    .await
    .expect(StatusCode::CREATED);

    let event = event_of_type(&app, "doc.comment.created").await;
    let mut expected = vec![owner.to_string(), mentioned.to_string()];
    expected.sort();
    assert_eq!(targets(&event), expected);
}
