//! P1 线程（threads）的失败用例（TDD 红阶段）。
//!
//! 约定（对齐 Matrix 的 threads）：
//! - 任意消息可作为线程根，回复带 `threadRootId`；
//! - 线程回复不出现在主消息流（`GET /messages` 只返回根消息），
//!   根消息带 `threadReplyCount`；
//! - `GET /conversations/{cid}/messages/{mid}/thread` 返回该根下的回复（按 seq 升序）；
//! - 线程根必须在同一会话（否则 422 `IM_VALIDATION`），且不能再嵌套线程（400 `IM_THREAD_NESTED`）；
//! - 只有会话成员可读写线程。

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
        Some(&json!({ "type": "group", "name": "线程测试群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发送消息，可选线程根。
async fn send(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    text: &str,
    thread_root: Option<&str>,
) -> TestResponse {
    let mut body = json!({ "type": "text", "content": { "text": text } });
    if let Some(root) = thread_root {
        body["threadRootId"] = json!(root);
    }
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        Some(&body),
    )
    .await
}

/// 读取主消息流。
async fn history(app: &TestApp, token: &str, conversation_id: &str) -> Value {
    let response = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        None,
    )
    .await;
    response.expect(StatusCode::OK)
}

/// 读取线程回复。
async fn thread(app: &TestApp, token: &str, conversation_id: &str, root: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{root}/thread"),
        Some(token),
        None,
    )
    .await
}

#[tokio::test]
async fn thread_replies_are_hidden_from_main_history_and_counted() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let root = send(&app, &owner_token, &conversation_id, "线程根", None)
        .await
        .expect(StatusCode::CREATED);
    let root_id = root["id"].as_str().unwrap().to_string();
    assert_eq!(root["threadReplyCount"], 0);
    assert!(root["threadRootId"].is_null());

    let first = send(
        &app,
        &member_token,
        &conversation_id,
        "第一条回复",
        Some(&root_id),
    )
    .await;
    let first = first.expect(StatusCode::CREATED);
    assert_eq!(first["threadRootId"], root_id);

    send(
        &app,
        &owner_token,
        &conversation_id,
        "第二条回复",
        Some(&root_id),
    )
    .await
    .expect(StatusCode::CREATED);

    // 主消息流只有根消息，且计数为 2
    let list = history(&app, &member_token, &conversation_id).await;
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 1, "线程回复不应出现在主消息流：{list}");
    assert_eq!(rows[0]["id"], root_id);
    assert_eq!(rows[0]["threadReplyCount"], 2);

    // 线程接口按 seq 升序返回回复
    let replies = thread(&app, &member_token, &conversation_id, &root_id).await;
    let replies = replies.expect(StatusCode::OK);
    let texts: Vec<&str> = replies
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["content"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, vec!["第一条回复", "第二条回复"]);
    assert_eq!(replies[0]["threadRootId"], root_id);
}

#[tokio::test]
async fn thread_root_must_belong_to_conversation() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let other_conversation = create_group(&app, &owner_token, &[member]).await;

    let foreign = send(&app, &owner_token, &other_conversation, "别的会话", None)
        .await
        .expect(StatusCode::CREATED);
    let foreign_id = foreign["id"].as_str().unwrap().to_string();

    let denied = send(
        &app,
        &member_token,
        &conversation_id,
        "跨会话线程",
        Some(&foreign_id),
    )
    .await;
    let body = denied.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // 根消息不存在
    let missing = send(
        &app,
        &member_token,
        &conversation_id,
        "不存在的根",
        Some(&Uuid::now_v7().to_string()),
    )
    .await;
    missing.expect(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn nested_threads_are_rejected() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let root_id = send(&app, &owner_token, &conversation_id, "根", None)
        .await
        .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let reply_id = send(&app, &owner_token, &conversation_id, "回复", Some(&root_id))
        .await
        .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let denied = send(
        &app,
        &owner_token,
        &conversation_id,
        "套娃线程",
        Some(&reply_id),
    )
    .await;
    let body = denied.expect(StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "IM_THREAD_NESTED");
}

#[tokio::test]
async fn thread_is_member_only() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let root_id = send(&app, &owner_token, &conversation_id, "根", None)
        .await
        .expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    thread(&app, &outsider_token, &conversation_id, &root_id)
        .await
        .expect(StatusCode::FORBIDDEN);
    send(
        &app,
        &outsider_token,
        &conversation_id,
        "外人回复线程",
        Some(&root_id),
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
