//! P1 消息编辑与编辑历史的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /conversations/{cid}/messages/{mid}`，请求体 `{ "content": {...} }`；
//! - 仅发送者本人可编辑自己的消息，其他人（含群主/管理员）一律 403；
//! - 首次编辑前把原内容补录为 `version=1`，每次编辑追加一版（`version` 递增）；
//! - 编辑后消息 `editedAt` 非空，`seq` 不变（不重排会话）；
//! - 已撤回消息不可编辑（400 `IM_MESSAGE_RECALLED`）；
//! - `GET /conversations/{cid}/messages/{mid}/revisions` 返回版本快照（按版本倒序），仅会话成员可读。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// 建群（创建者为群主）。
async fn create_group(app: &TestApp, token: &str, members: &[Uuid], name: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "group", "name": name, "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发送一条文本消息并返回消息 ID。
async fn send_text(app: &TestApp, token: &str, conversation_id: &str, text: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(token),
        Some(&json!({
            "type": "text",
            "content": { "text": text },
            "clientMsgId": Uuid::now_v7().to_string(),
        })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 编辑消息。
async fn patch_message(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    message_id: &str,
    content: Value,
) -> TestResponse {
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{message_id}"),
        Some(token),
        Some(&json!({ "content": content })),
    )
    .await
}

/// 读取编辑历史。
async fn get_revisions(
    app: &TestApp,
    token: &str,
    conversation_id: &str,
    message_id: &str,
) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/{message_id}/revisions"),
        Some(token),
        None,
    )
    .await
}

/// 取某版本的正文。
fn revision_text(revisions: &Value, version: i64) -> String {
    revisions
        .as_array()
        .expect("版本列表")
        .iter()
        .find(|row| row["version"] == version)
        .unwrap_or_else(|| panic!("缺少 v{version} 版本：{revisions}"))["content"]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn sender_edits_own_message() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "原始内容").await;

    let edited = patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        json!({ "text": "改后内容" }),
    )
    .await;
    let body = edited.expect(StatusCode::OK);
    assert_eq!(body["id"], message_id);
    assert_eq!(body["content"]["text"], "改后内容");
    assert!(
        !body["editedAt"].is_null(),
        "编辑后 editedAt 应非空：{body}"
    );
    assert_eq!(body["seq"], 1, "编辑不应改变会话内序号");

    // 历史消息读到的是最新正文
    let history = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&member_token),
        None,
    )
    .await;
    let list = history.expect(StatusCode::OK);
    assert_eq!(list[0]["content"]["text"], "改后内容");
    assert!(!list[0]["editedAt"].is_null());
}

#[tokio::test]
async fn first_edit_backfills_original_content() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "第一版").await;

    patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        json!({ "text": "第二版" }),
    )
    .await
    .expect(StatusCode::OK);

    let history = get_revisions(&app, &owner_token, &conversation_id, &message_id).await;
    let revisions = history.expect(StatusCode::OK);
    let rows = revisions.as_array().expect("版本列表");
    assert_eq!(rows.len(), 2, "首次编辑应补录原内容：{revisions}");
    assert_eq!(rows[0]["version"], 2, "版本按倒序返回");
    assert_eq!(revision_text(&revisions, 1), "第一版");
    assert_eq!(revision_text(&revisions, 2), "第二版");
    assert_eq!(rows[1]["editorId"], owner.to_string());
}

#[tokio::test]
async fn second_edit_appends_another_revision() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "v0").await;

    for text in ["v1", "v2"] {
        patch_message(
            &app,
            &owner_token,
            &conversation_id,
            &message_id,
            json!({ "text": text }),
        )
        .await
        .expect(StatusCode::OK);
    }

    let history = get_revisions(&app, &owner_token, &conversation_id, &message_id).await;
    let revisions = history.expect(StatusCode::OK);
    let versions: Vec<i64> = revisions
        .as_array()
        .expect("版本列表")
        .iter()
        .map(|row| row["version"].as_i64().unwrap())
        .collect();
    assert_eq!(versions, vec![3, 2, 1]);
    assert_eq!(revision_text(&revisions, 1), "v0");
    assert_eq!(revision_text(&revisions, 2), "v1");
    assert_eq!(revision_text(&revisions, 3), "v2");
}

#[tokio::test]
async fn non_sender_cannot_edit_others_message() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider_role = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_role_token = issue_token(&app, outsider_role);
    let conversation_id = create_group(
        &app,
        &owner_token,
        &[member, outsider_role],
        "编辑测试群",
    )
    .await;

    // 群主把成员设为管理员，管理员也不能改别人的消息
    request(
        &app.app,
        "POST",
        &format!(
            "/api/v1/im/conversations/{conversation_id}/members/{member}/role"
        ),
        Some(&owner_token),
        Some(&json!({ "role": "admin" })),
    )
    .await
    .expect(StatusCode::OK);

    let owner_message = send_text(&app, &owner_token, &conversation_id, "群主发言").await;
    let denied = patch_message(
        &app,
        &member_token,
        &conversation_id,
        &owner_message,
        json!({ "text": "管理员篡改" }),
    )
    .await;
    let body = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "IM_FORBIDDEN");

    // 群主同样不能改成员的消息
    let member_message = send_text(&app, &member_token, &conversation_id, "成员发言").await;
    let denied = patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &member_message,
        json!({ "text": "群主篡改" }),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    // 正文未被改动
    let history = request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&outsider_role_token),
        None,
    )
    .await;
    let list = history.expect(StatusCode::OK);
    assert_eq!(list[0]["content"]["text"], "群主发言");
}

#[tokio::test]
async fn recalled_message_cannot_be_edited() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "马上撤回").await;

    request(
        &app.app,
        "POST",
        &format!(
            "/api/v1/im/conversations/{conversation_id}/messages/{message_id}/recall"
        ),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::OK);

    let denied = patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        json!({ "text": "撤回后改" }),
    )
    .await;
    let body = denied.expect(StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "IM_MESSAGE_RECALLED");
}

#[tokio::test]
async fn edit_rejects_empty_content() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "有内容").await;

    let denied = patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        Value::Null,
    )
    .await;
    let body = denied.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");
}

#[tokio::test]
async fn revisions_are_member_only() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "内容").await;

    patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        json!({ "text": "改过" }),
    )
    .await
    .expect(StatusCode::OK);

    let denied = get_revisions(&app, &outsider_token, &conversation_id, &message_id).await;
    let body = denied.expect(StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "IM_NOT_MEMBER");

    let denied = patch_message(
        &app,
        &outsider_token,
        &conversation_id,
        &message_id,
        json!({ "text": "外人改" }),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unchanged_content_does_not_create_revision() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member], "编辑测试群").await;
    let message_id = send_text(&app, &owner_token, &conversation_id, "原样").await;

    // 正文完全一致：幂等返回，不写版本，不标 editedAt
    let same = patch_message(
        &app,
        &owner_token,
        &conversation_id,
        &message_id,
        json!({ "text": "原样" }),
    )
    .await;
    let body = same.expect(StatusCode::OK);
    assert!(body["editedAt"].is_null(), "无变化的编辑不应标记 editedAt");

    let history = get_revisions(&app, &owner_token, &conversation_id, &message_id).await;
    let revisions = history.expect(StatusCode::OK);
    assert_eq!(
        revisions.as_array().expect("版本列表").len(),
        0,
        "无变化不应产生版本：{revisions}"
    );
}
