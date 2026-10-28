//! P2 会话置顶与消息收藏的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /conversations/{cid}/pin` body `{ "pinned": true }`：设置自己的置顶，返回 `{pinned}`；
//! - `POST /conversations/{cid}/messages/{mid}/star`：切换收藏，返回 `{starred}`；
//! - 消息列表返回 `starred`（当前用户是否收藏）；
//! - `GET /messages/starred`：我收藏的消息（按收藏时间倒序，仅自己可见）；
//! - 非成员 403。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

/// 建群。
async fn create_group(app: &TestApp, token: &str, members: &[Uuid]) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "group", "name": "置顶收藏群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发文本消息。
async fn send(app: &TestApp, token: &str, conversation_id: &str, text: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        Some(&json!({ "type": "text", "content": { "text": text } })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 设置置顶。
async fn set_pin(app: &TestApp, token: &str, conversation_id: &str, pinned: bool) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/pin"),
        Some(token),
        Some(&json!({ "pinned": pinned })),
    )
    .await
}

/// 切换收藏。
async fn toggle_star(app: &TestApp, token: &str, conversation_id: &str, message_id: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{message_id}/star"),
        Some(token),
        None,
    )
    .await
}

/// 会话列表里的某会话。
fn conversation_of<'a>(list: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .expect("会话存在")
}

#[tokio::test]
async fn pin_is_per_member() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let updated = set_pin(&app, &member_token, &conversation_id, true).await;
    let body = updated.expect(StatusCode::OK);
    assert_eq!(body["pinned"], true);

    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(conversation_of(&list.expect(StatusCode::OK), &conversation_id)["pinned"], true);

    // 另一位成员不受影响
    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(conversation_of(&list.expect(StatusCode::OK), &conversation_id)["pinned"], false);

    // 取消置顶
    set_pin(&app, &member_token, &conversation_id, false)
        .await
        .expect(StatusCode::OK);
    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(conversation_of(&list.expect(StatusCode::OK), &conversation_id)["pinned"], false);
}

#[tokio::test]
async fn star_toggles_and_lists_my_favorites() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let first = send(&app, &owner_token, &conversation_id, "重要：周五交材料").await;
    let second = send(&app, &owner_token, &conversation_id, "闲聊").await;

    let starred = toggle_star(&app, &member_token, &conversation_id, &first).await;
    let body = starred.expect(StatusCode::OK);
    assert_eq!(body["starred"], true);

    // 消息列表带 starred 标记（只对收藏者本人为 true）
    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&member_token),
        None,
    )
    .await;
    let rows = list.expect(StatusCode::OK);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == first)
        .unwrap();
    assert_eq!(row["starred"], true);
    let other_row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == second)
        .unwrap();
    assert_eq!(other_row["starred"], false);

    let list = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    let rows = list.expect(StatusCode::OK);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == first)
        .unwrap();
    assert_eq!(row["starred"], false, "别人的收藏不可见");

    // 收藏列表：只返回自己的收藏，带 conversationId
    let favorites = request(
        &app.app,
        "GET",
        "/api/v1/im/messages/starred",
        Some(&member_token),
        None,
    )
    .await;
    let favorites = favorites.expect(StatusCode::OK);
    let rows = favorites.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], first);
    assert_eq!(rows[0]["conversationId"], conversation_id);
    assert_eq!(rows[0]["starred"], true);

    let owner_favorites = request(
        &app.app,
        "GET",
        "/api/v1/im/messages/starred",
        Some(&owner_token),
        None,
    )
    .await;
    assert!(owner_favorites
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());

    // 再点一次取消收藏，收藏列表清空
    let unstarred = toggle_star(&app, &member_token, &conversation_id, &first).await;
    assert_eq!(unstarred.expect(StatusCode::OK)["starred"], false);
    let favorites = request(
        &app.app,
        "GET",
        "/api/v1/im/messages/starred",
        Some(&member_token),
        None,
    )
    .await;
    assert!(favorites
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn pin_and_star_require_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let message_id = send(&app, &owner_token, &conversation_id, "内容").await;

    set_pin(&app, &outsider_token, &conversation_id, true)
        .await
        .expect(StatusCode::FORBIDDEN);
    toggle_star(&app, &outsider_token, &conversation_id, &message_id)
        .await
        .expect(StatusCode::FORBIDDEN);
}
