//! P2 会话导出与审计的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /conversations/{cid}/export?format=md|json`：成员可导出整个会话；
//! - Markdown：标题、按 seq 正序的每条消息（发送者、时间、正文、编辑标记、线程缩进）；
//! - JSON：`{ conversation: {...}, messages: [...] }`，消息含 seq/senderId/content/editedAt/threadRootId；
//! - 非成员 403，不支持的格式 422；
//! - 每次导出写一条审计记录（`change_log`：entity=im_conversation，action=export）。

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

/// 原始文本响应。
struct TextResponse {
    status: StatusCode,
    content_type: Option<String>,
    disposition: Option<String>,
    body: String,
}

/// 取原始文本响应。
async fn get_text(app: &TestApp, uri: &str, token: &str) -> TextResponse {
    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("请求"),
        )
        .await
        .expect("执行请求");
    let status = response.status();
    let header_of = |name: header::HeaderName| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    };
    let content_type = header_of(header::CONTENT_TYPE);
    let disposition = header_of(header::CONTENT_DISPOSITION);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("响应体")
        .to_bytes();
    TextResponse {
        status,
        content_type,
        disposition,
        body: String::from_utf8_lossy(&body).to_string(),
    }
}

/// 建群。
async fn create_group(app: &TestApp, token: &str, members: &[Uuid]) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/im/conversations",
        Some(token),
        Some(&json!({ "type": "group", "name": "导出测试群", "memberIds": members })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// 发文本消息，返回消息 ID。
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

#[tokio::test]
async fn export_markdown_contains_messages_in_order() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    send(&app, &owner_token, &conversation_id, "第一条消息").await;
    let second = send(&app, &member_token, &conversation_id, "第二条消息").await;
    // 引用 + 线程回复，验证导出结构
    request(
        &app.app,
        "POST",
        &format!("/api/v1/im/conversations/{conversation_id}/messages"),
        Some(&member_token),
        Some(&json!({
            "type": "text",
            "content": { "text": "线程里的回复" },
            "threadRootId": second,
        })),
    )
    .await
    .expect(StatusCode::CREATED);

    let export = get_text(
        &app,
        &format!("/api/v1/im/conversations/{conversation_id}/export?format=md"),
        &owner_token,
    )
    .await;
    assert_eq!(export.status, StatusCode::OK);
    assert!(
        export
            .content_type
            .as_deref()
            .unwrap_or_default()
            .starts_with("text/markdown"),
        "content-type={:?}",
        export.content_type
    );
    assert!(
        export
            .disposition
            .as_deref()
            .unwrap_or_default()
            .contains("attachment"),
        "应作为附件下载"
    );
    assert!(export.body.contains("# 导出测试群"), "{}", export.body);
    assert!(export.body.contains("第一条消息"));
    assert!(export.body.contains("第二条消息"));
    assert!(export.body.contains("线程里的回复"));
    let first_pos = export.body.find("第一条消息").unwrap();
    let second_pos = export.body.find("第二条消息").unwrap();
    assert!(first_pos < second_pos, "应按时间顺序导出");
}

#[tokio::test]
async fn export_json_has_conversation_and_messages() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    let message_id = send(&app, &owner_token, &conversation_id, "导出的内容").await;

    let export = get_text(
        &app,
        &format!("/api/v1/im/conversations/{conversation_id}/export?format=json"),
        &member_token,
    )
    .await;
    assert_eq!(export.status, StatusCode::OK);
    assert!(export
        .content_type
        .as_deref()
        .unwrap_or_default()
        .starts_with("application/json"));
    let body: serde_json::Value = serde_json::from_str(&export.body).expect("合法 JSON");
    assert_eq!(body["conversation"]["id"], conversation_id);
    assert_eq!(body["conversation"]["name"], "导出测试群");
    let messages = body["messages"].as_array().expect("消息数组");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["id"], message_id);
    assert_eq!(messages[0]["content"]["text"], "导出的内容");
    assert_eq!(messages[0]["senderId"], owner.to_string());
}

#[tokio::test]
async fn export_requires_membership_and_valid_format() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;

    let denied = get_text(
        &app,
        &format!("/api/v1/im/conversations/{conversation_id}/export?format=md"),
        &outsider_token,
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    let invalid = get_text(
        &app,
        &format!("/api/v1/im/conversations/{conversation_id}/export?format=pdf"),
        &owner_token,
    )
    .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn export_is_audited() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[member]).await;
    send(&app, &owner_token, &conversation_id, "内容").await;

    get_text(
        &app,
        &format!("/api/v1/im/conversations/{conversation_id}/export?format=json"),
        &owner_token,
    )
    .await;

    let entries = club_bus::audit::list_for(&app.state.db, "im_conversation", &conversation_id, 10)
        .await
        .expect("读取审计");
    assert_eq!(entries.len(), 1, "应记录一次导出");
    assert_eq!(entries[0].action, "export");
    assert_eq!(entries[0].actor_id, Some(owner));
    assert_eq!(entries[0].after.as_ref().unwrap()["format"], "json");
}
