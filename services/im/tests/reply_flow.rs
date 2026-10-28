//! 引用回复的回归用例：引用关系落库、回读，以及跨会话引用被拒。

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
        Some(&json!({ "type": "group", "name": "引用测试群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发送文本消息，返回响应体。
async fn send(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    body: serde_json::Value,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        Some(&body),
    )
    .await
}

#[tokio::test]
async fn reply_is_persisted_and_returned() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let original = send(
        &app,
        &owner_token,
        &conversation_id,
        json!({ "type": "text", "content": { "text": "原始消息" } }),
    )
    .await;
    let original_id = original.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let reply = send(
        &app,
        &member_token,
        &conversation_id,
        json!({
            "type": "text",
            "content": { "text": "回复你" },
            "replyToId": original_id,
        }),
    )
    .await;
    let body = reply.expect(StatusCode::CREATED);
    assert_eq!(body["replyToId"], original_id);

    // 历史回读保留引用关系
    let history = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    let list = history.expect(StatusCode::OK);
    let reply_row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == body["id"])
        .expect("回复消息存在");
    assert_eq!(reply_row["replyToId"], original_id);
}

#[tokio::test]
async fn reply_to_other_conversation_is_rejected() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let other_conversation = create_group(&app, &owner_token, &[member]).await;

    let foreign = send(
        &app,
        &owner_token,
        &other_conversation,
        json!({ "type": "text", "content": { "text": "另一个会话" } }),
    )
    .await;
    let foreign_id = foreign.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let denied = send(
        &app,
        &member_token,
        &conversation_id,
        json!({
            "type": "text",
            "content": { "text": "跨会话引用" },
            "replyToId": foreign_id,
        }),
    )
    .await;
    let body = denied.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");
}
