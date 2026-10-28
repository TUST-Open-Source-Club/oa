//! P0 文档搜索失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /api/v1/doc/search?q=&limit=`：在我可读的空间里搜标题与正文；
//! - 结果 `{ id, spaceId, spaceName, title, snippet, updatedAt }`，按更新时间倒序；
//! - viewer 搜不到 private 文档；已删除文档不出现；空关键词 422；
//! - `limit` 默认 20、上限 50。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str, name: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": name, "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_page(app: &TestApp, token: &str, space_id: &str, title: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&json!({ "kind": "page", "title": title })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn write_content(app: &TestApp, token: &str, space_id: &str, node_id: &str, md: &str) {
    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node_id}/content"),
        Some(token),
        Some(&json!({ "contentMd": md, "baseVersion": 1 })),
    )
    .await
    .expect(StatusCode::OK);
}

async fn add_member(app: &TestApp, space_id: &str, user: Uuid, role: &str) {
    use doc_service::entity::space_member;
    space_member::ActiveModel {
        space_id: Set(space_id.parse().unwrap()),
        user_id: Set(user),
        role: Set(role.to_string()),
        joined_at: Set(app.state.now().fixed_offset()),
    }
    .insert(&app.state.db)
    .await
    .expect("加成员");
}

async fn search(app: &TestApp, token: &str, q: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/search?q={q}"),
        Some(token),
        None,
    )
    .await
}

/// 标题列表。
fn titles(body: &serde_json::Value) -> Vec<String> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|row| row["title"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn search_matches_title_and_body() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token, "搜索库").await;

    let plan = create_page(&app, &token, &space_id, "秋季招新方案").await;
    write_content(
        &app,
        &token,
        &space_id,
        &plan,
        "# 招新目标\n\n计划招募 60 名新成员，重点在计算机与设计方向。",
    )
    .await;
    let budget = create_page(&app, &token, &space_id, "年度预算").await;
    write_content(&app, &token, &space_id, &budget, "场地费用与宣传费用合计 8000 元。").await;

    // 标题命中
    let by_title = search(&app, &token, "招新").await;
    let body = by_title.expect(StatusCode::OK);
    assert_eq!(titles(&body), vec!["秋季招新方案"]);
    assert_eq!(body[0]["spaceId"], space_id);
    assert_eq!(body[0]["spaceName"], "搜索库");

    // 正文命中，snippet 带上下文
    let by_body = search(&app, &token, "宣传费用").await;
    let body = by_body.expect(StatusCode::OK);
    assert_eq!(titles(&body), vec!["年度预算"]);
    assert!(
        body[0]["snippet"].as_str().unwrap().contains("宣传费用"),
        "snippet={}",
        body[0]["snippet"]
    );
}

#[tokio::test]
async fn search_is_scoped_to_my_spaces() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let other = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let other_token = issue_token(&app, other);
    let mine = create_space(&app, &owner_token, "我的库").await;
    let theirs = create_space(&app, &other_token, "别人的库").await;

    let my_page = create_page(&app, &owner_token, &mine, "项目复盘").await;
    write_content(&app, &owner_token, &mine, &my_page, "复盘内容").await;
    let their_page = create_page(&app, &other_token, &theirs, "项目计划").await;
    write_content(&app, &other_token, &theirs, &their_page, "计划内容").await;

    let body = search(&app, &owner_token, "项目").await.expect(StatusCode::OK);
    assert_eq!(titles(&body), vec!["项目复盘"]);
}

#[tokio::test]
async fn search_hides_private_and_deleted_docs() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token, "权限库").await;
    add_member(&app, &space_id, viewer, "viewer").await;

    let shared = create_page(&app, &owner_token, &space_id, "公开招新说明").await;
    write_content(&app, &owner_token, &space_id, &shared, "招新说明").await;
    let private = create_page(&app, &owner_token, &space_id, "私密招新预算").await;
    write_content(&app, &owner_token, &space_id, &private, "招新预算明细").await;
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{private}"),
        Some(&owner_token),
        Some(&json!({ "visibility": "private" })),
    )
    .await
    .expect(StatusCode::OK);

    // viewer 只搜到公开那篇
    let body = search(&app, &viewer_token, "招新").await.expect(StatusCode::OK);
    assert_eq!(titles(&body), vec!["公开招新说明"]);

    // 编辑者两篇都能搜到
    let body = search(&app, &owner_token, "招新").await.expect(StatusCode::OK);
    assert_eq!(titles(&body).len(), 2);

    // 删除后不再出现
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{shared}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let body = search(&app, &owner_token, "招新").await.expect(StatusCode::OK);
    assert_eq!(titles(&body), vec!["私密招新预算"]);
}

#[tokio::test]
async fn search_validates_query_and_limit() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token, "校验库").await;
    for index in 0..3 {
        let page = create_page(&app, &token, &space_id, &format!("关键词 {index}")).await;
        write_content(&app, &token, &space_id, &page, "关键词正文").await;
    }

    let empty = search(&app, &token, "%20%20").await;
    let body = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "DOC_VALIDATION");

    let limited = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/search?q=%E5%85%B3%E9%94%AE%E8%AF%8D&limit=2"),
        Some(&token),
        None,
    )
    .await;
    let body = limited.expect(StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 2);
}
