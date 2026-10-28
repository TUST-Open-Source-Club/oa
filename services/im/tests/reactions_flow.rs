//! P1 表情回应的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /conversations/{cid}/messages/{mid}/reactions`，body `{ "emoji": "👍" }`，**切换**语义：
//!   已回应则取消，返回该消息最新的回应汇总；
//! - 汇总项 `{ emoji, count, mine }`，`mine` 表示当前用户是否回应过；
//! - 消息列表（`GET /messages`）每条消息带 `reactions`；
//! - 仅会话成员可回应（403），已撤回消息不可回应（400），空 emoji 422。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// 建群。
async fn create_group(app: &TestApp, token: &str, members: &[Uuid]) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "group", "name": "回应测试群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发一条文本消息，返回消息 ID。
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

/// 切换表情回应。
async fn toggle(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    message_id: &str,
    emoji: &str,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{message_id}/reactions"),
        Some(token),
        Some(&json!({ "emoji": emoji })),
    )
    .await
}

/// 取消息列表里的某条消息。
async fn find_in_history(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    message_id: &str,
) -> Value {
    let response = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        None,
    )
    .await;
    let list = response.expect(StatusCode::OK);
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == message_id)
        .cloned()
        .expect("消息存在")
}

#[tokio::test]
async fn toggle_reaction_counts_and_flags_mine() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let message_id = send(&app, &owner_token, &conversation_id, "给他点个赞").await;

    let first = toggle(&app, &member_token, &conversation_id, &message_id, "👍").await;
    let body = first.expect(StatusCode::OK);
    assert_eq!(body["reactions"][0]["emoji"], "👍");
    assert_eq!(body["reactions"][0]["count"], 1);
    assert_eq!(body["reactions"][0]["mine"], true);

    // 另一个人看到 count=2、mine=false
    let second = toggle(&app, &owner_token, &conversation_id, &message_id, "👍").await;
    second.expect(StatusCode::OK);

    let seen_by_owner = find_in_history(&app, &owner_token, &conversation_id, &message_id).await;
    assert_eq!(seen_by_owner["reactions"][0]["count"], 2);
    assert_eq!(seen_by_owner["reactions"][0]["mine"], true);

    let outsider_view = find_in_history(&app, &member_token, &conversation_id, &message_id).await;
    assert_eq!(outsider_view["reactions"][0]["count"], 2);
    assert_eq!(outsider_view["reactions"][0]["mine"], true);

    // 再点一次取消
    let canceled = toggle(&app, &member_token, &conversation_id, &message_id, "👍").await;
    let body = canceled.expect(StatusCode::OK);
    assert_eq!(body["reactions"][0]["count"], 1);
    assert_eq!(body["reactions"][0]["mine"], false);
}

#[tokio::test]
async fn reactions_are_per_emoji() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let message_id = send(&app, &owner_token, &conversation_id, "多表情").await;

    toggle(&app, &member_token, &conversation_id, &message_id, "👍")
        .await
        .expect(StatusCode::OK);
    toggle(&app, &owner_token, &conversation_id, &message_id, "🎉")
        .await
        .expect(StatusCode::OK);

    let row = find_in_history(&app, &member_token, &conversation_id, &message_id).await;
    let reactions = row["reactions"].as_array().unwrap();
    assert_eq!(reactions.len(), 2);
    assert!(reactions.iter().any(|item| item["emoji"] == "👍" && item["count"] == 1));
    assert!(reactions.iter().any(|item| item["emoji"] == "🎉" && item["count"] == 1));
}

#[tokio::test]
async fn reaction_validation_and_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let message_id = send(&app, &owner_token, &conversation_id, "校验").await;

    // 非成员
    toggle(&app, &outsider_token, &conversation_id, &message_id, "👍")
        .await
        .expect(StatusCode::FORBIDDEN);

    // 空 emoji
    let empty = toggle(&app, &owner_token, &conversation_id, &message_id, "  ").await;
    let body = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // 已撤回消息
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{message_id}/recall"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::OK);
    let denied = toggle(&app, &owner_token, &conversation_id, &message_id, "👍").await;
    let body = denied.expect(StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "IM_MESSAGE_RECALLED");
}
