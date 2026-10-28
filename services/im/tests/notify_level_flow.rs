//! P1 会话通知规则的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 每个成员对每个会话有独立通知级别：`all`（全部）/ `mentions`（仅 @我）/ `mute`（静音）；
//! - `POST /conversations/{cid}/notify`，body `{ "level": "mentions" }`，返回当前级别；
//! - 会话列表返回调用者自己的 `notifyLevel`；
//! - 级别不合法 422 `IM_VALIDATION`；非成员 403。

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
        Some(&json!({ "type": "group", "name": "通知规则群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 设置通知级别。
async fn set_level(app: &TestApp, token: &str, conversation_id: &str, level: &str) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/notify"),
        Some(token),
        Some(&json!({ "level": level })),
    )
    .await
}

/// 取会话列表里的某个会话。
fn conversation_of<'a>(
    list: &'a serde_json::Value,
    conversation_id: &str,
) -> &'a serde_json::Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == conversation_id)
        .expect("会话存在")
}

#[tokio::test]
async fn notify_level_is_per_member_and_reflected_in_list() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    // 默认全部通知
    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&owner_token),
        None,
    )
    .await;
    let body = list.expect(StatusCode::OK);
    assert_eq!(
        conversation_of(&body, &conversation_id)["notifyLevel"],
        "all"
    );

    // 成员改成仅 @我
    let updated = set_level(&app, &member_token, &conversation_id, "mentions").await;
    let body = updated.expect(StatusCode::OK);
    assert_eq!(body["level"], "mentions");

    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&member_token),
        None,
    )
    .await;
    let body = list.expect(StatusCode::OK);
    assert_eq!(
        conversation_of(&body, &conversation_id)["notifyLevel"],
        "mentions"
    );

    // 群主的设置不受影响
    let list = request(
        &app.app,
        "GET",
        "/api/v1/im/conversations",
        Some(&owner_token),
        None,
    )
    .await;
    let body = list.expect(StatusCode::OK);
    assert_eq!(
        conversation_of(&body, &conversation_id)["notifyLevel"],
        "all"
    );

    // 静音
    set_level(&app, &member_token, &conversation_id, "mute")
        .await
        .expect(StatusCode::OK);
}

#[tokio::test]
async fn notify_level_validates_input_and_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let invalid = set_level(&app, &owner_token, &conversation_id, "sometimes").await;
    let body = invalid.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    set_level(&app, &outsider_token, &conversation_id, "mute")
        .await
        .expect(StatusCode::FORBIDDEN);
}
