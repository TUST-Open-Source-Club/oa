//! P1 消息搜索的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /conversations/{cid}/messages/search?q=关键词`：会话内搜索，按 seq 倒序；
//! - `GET /search/messages?q=关键词`：跨会话搜索，仅返回调用者是成员的会话；
//! - 匹配 `content.text`，大小写不敏感的子串匹配；
//! - 空关键词 422 `IM_VALIDATION`；非成员访问会话搜索 403；
//! - 结果里带 `conversationId`，方便前端跳转。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// 建群。
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

/// 会话内搜索。
async fn search_in(app: &TestApp, token: &str, conversation_id: &str, q: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/im/conversations/{conversation_id}/messages/search?q={q}"),
        Some(token),
        None,
    )
    .await
}

/// 跨会话搜索。
async fn search_all(app: &TestApp, token: &str, q: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/im/search/messages?q={q}"),
        Some(token),
        None,
    )
    .await
}

/// 结果里的正文列表。
fn texts(results: &Value) -> Vec<String> {
    results
        .as_array()
        .expect("结果数组")
        .iter()
        .map(|row| {
            row["content"]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn conversation_search_matches_substring_case_insensitively() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let conversation_id = create_group(&app, &owner_token, &[member], "搜索测试群").await;

    send(&app, &owner_token, &conversation_id, "周五开 Budget 评审会").await;
    send(&app, &member_token, &conversation_id, "预算表我整理好了").await;
    send(&app, &owner_token, &conversation_id, "收到，谢谢").await;

    let hit = search_in(&app, &member_token, &conversation_id, "budget").await;
    let results = hit.expect(StatusCode::OK);
    assert_eq!(
        texts(&results),
        vec!["周五开 Budget 评审会"],
        "大小写不敏感匹配"
    );
    assert_eq!(results[0]["conversationId"], conversation_id);

    let second = search_in(&app, &owner_token, &conversation_id, "预算").await;
    let results = second.expect(StatusCode::OK);
    assert_eq!(texts(&results), vec!["预算表我整理好了"]);

    // 倒序：两条命中时后发的在前
    let multi = search_in(&app, &owner_token, &conversation_id, "我").await;
    let results = multi.expect(StatusCode::OK);
    let seqs: Vec<i64> = results
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["seq"].as_i64().unwrap())
        .collect();
    let mut sorted = seqs.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(seqs, sorted, "按 seq 倒序");
}

#[tokio::test]
async fn conversation_search_is_scoped_and_member_only() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member], "会话 A").await;
    let other_conversation = create_group(&app, &owner_token, &[member], "会话 B").await;

    send(&app, &owner_token, &conversation_id, "A 里的关键词 alpha").await;
    send(
        &app,
        &owner_token,
        &other_conversation,
        "B 里的关键词 alpha",
    )
    .await;

    let hit = search_in(&app, &owner_token, &conversation_id, "alpha").await;
    let results = hit.expect(StatusCode::OK);
    assert_eq!(texts(&results), vec!["A 里的关键词 alpha"]);

    search_in(&app, &outsider_token, &conversation_id, "alpha")
        .await
        .expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn global_search_only_returns_my_conversations() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let outsider_token = issue_token(&app, outsider);
    let conversation_id = create_group(&app, &owner_token, &[member], "全员群").await;
    let private_conversation =
        create_group(&app, &owner_token, &[Uuid::now_v7()], "小范围群").await;

    send(&app, &owner_token, &conversation_id, "公告：年会报名开始").await;
    send(
        &app,
        &owner_token,
        &private_conversation,
        "年会预算内部讨论",
    )
    .await;

    let member_hit = search_all(&app, &member_token, "年会").await;
    let results = member_hit.expect(StatusCode::OK);
    assert_eq!(texts(&results), vec!["公告：年会报名开始"]);
    assert_eq!(results[0]["conversationId"], conversation_id);

    let owner_hit = search_all(&app, &owner_token, "年会").await;
    let results = owner_hit.expect(StatusCode::OK);
    assert_eq!(results.as_array().unwrap().len(), 2);

    let outsider_hit = search_all(&app, &outsider_token, "年会").await;
    let results = outsider_hit.expect(StatusCode::OK);
    assert!(results.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_validates_query_and_limit() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let conversation_id = create_group(&app, &owner_token, &[Uuid::now_v7()], "校验群").await;

    for text in ["一", "二", "三"] {
        send(
            &app,
            &owner_token,
            &conversation_id,
            &format!("关键词 {text}"),
        )
        .await;
    }

    // 空关键词
    let empty = search_in(&app, &owner_token, &conversation_id, "%20%20").await;
    let body = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "IM_VALIDATION");

    // limit 生效
    let limited = request(
        &app.app,
        "GET",
        &format!(
            "/api/v1/im/conversations/{conversation_id}/messages/search?q=%E5%85%B3%E9%94%AE%E8%AF%8D&limit=2"
        ),
        Some(&owner_token),
        None,
    )
    .await;
    let results = limited.expect(StatusCode::OK);
    assert_eq!(results.as_array().unwrap().len(), 2);
}
