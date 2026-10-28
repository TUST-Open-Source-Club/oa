//! P1「正在输入」的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `POST /conversations/{cid}/typing` 记录一次输入心跳（204），仅会话成员可调用；
//! - 会话列表返回 `typingUserIds`（近 5 秒内的心跳，含自己，前端自行过滤）；
//! - 心跳有 TTL，过期后不再出现。

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
        Some(&json!({ "type": "group", "name": "输入状态群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发送输入心跳。
async fn typing(app: &TestApp, token: &str, conversation_id: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/typing"),
        Some(token),
        None,
    )
    .await
}

/// 取会话列表里的某会话。
async fn conversation_of(app: &TestApp, token: &str, conversation_id: &str) -> serde_json::Value {
    let response = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(token),
        None,
    )
    .await;
    let list = response.expect(StatusCode::OK);
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == conversation_id)
        .cloned()
        .expect("会话存在")
}

#[tokio::test]
async fn typing_heartbeat_is_visible_to_members() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    typing(&app, &member_token, &conversation_id)
        .await
        .expect(StatusCode::NO_CONTENT);

    let seen = conversation_of(&app, &owner_token, &conversation_id).await;
    let ids: Vec<&str> = seen["typingUserIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![member.to_string().as_str()]);

    // 自己的列表里也会带上（前端过滤掉自己）
    let own = conversation_of(&app, &member_token, &conversation_id).await;
    assert!(own["typingUserIds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == &json!(member.to_string())));
}

#[tokio::test]
async fn typing_requires_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    typing(&app, &outsider_token, &conversation_id)
        .await
        .expect(StatusCode::FORBIDDEN);
}
