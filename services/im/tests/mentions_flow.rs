//! P0 `@` 提及与 `@所有人` 的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `mentions` 为字符串数组，成员用用户 ID，`@所有人` 用 `"@all"`；
//! - 提及必须是会话成员，否则 400；
//! - `@all` 仅群主/管理员可用（403），单聊不可用（400）。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

/// 建群并把创建者设为群主。
async fn create_group(app: &TestApp, token: &str, members: &[Uuid], name: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "group", "name": name, "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"].as_str().unwrap().to_string()
}

/// 建单聊。
async fn create_direct(app: &TestApp, token: &str, other: Uuid) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "direct", "memberIds": [other] })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"].as_str().unwrap().to_string()
}

/// 发送带 mentions 的文本消息。
async fn send_with_mentions(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    mentions: serde_json::Value,
) -> TestResponse {
    let payload = json!({
        "type": "text",
        "content": { "text": "请看这条" },
        "mentions": mentions,
        "clientMsgId": Uuid::now_v7().to_string(),
    });
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        Some(&payload),
    )
    .await
}

#[tokio::test]
async fn mention_member_is_persisted_and_returned() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member], "提及测试群").await;

    let sent = send_with_mentions(&app, &owner_token, &conversation_id, json!([member.to_string()])).await;
    let body = sent.expect(StatusCode::CREATED);
    assert_eq!(body["mentions"], json!([member.to_string()]));

    let history = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(
        history.expect(StatusCode::OK)[0]["mentions"],
        json!([member.to_string()])
    );
}

#[tokio::test]
async fn mention_non_member_is_rejected() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "提及测试群").await;

    let sent = send_with_mentions(&app, &owner_token, &conversation_id, json!([outsider.to_string()])).await;
    sent.expect(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn member_cannot_mention_all() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member], "提及测试群").await;

    let sent = send_with_mentions(&app, &member_token, &conversation_id, json!(["@all"])).await;
    sent.expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn owner_can_mention_all() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "提及测试群").await;

    let sent = send_with_mentions(&app, &owner_token, &conversation_id, json!(["@all"])).await;
    let body = sent.expect(StatusCode::CREATED);
    assert_eq!(body["mentions"], json!(["@all"]));
}

#[tokio::test]
async fn direct_chat_rejects_mention_all() {
    let app = spawn().await;
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let token_a = issue_token(&app, a);
    let conversation_id = create_direct(&app, &token_a, b).await;

    let sent = send_with_mentions(&app, &token_a, &conversation_id, json!(["@all"])).await;
    sent.expect(StatusCode::BAD_REQUEST);
}
