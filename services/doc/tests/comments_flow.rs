//! P1 文档评论失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /spaces/{id}/nodes/{nodeId}/comments`：成员可读，按时间正序；
//! - `POST .../comments`：`{ body, parentId?, anchor? }`，anchor 形如 `{ "heading": "时间安排" }`；
//! - `PATCH .../comments/{commentId}`：`{ resolved: true }` 解决/取消解决，
//!   评论作者或空间编辑者可操作；
//! - `DELETE .../comments/{commentId}`：作者或编辑者可删，子回复一并删除；
//! - 非成员 403，空正文 422。

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "评论库", "type": "team" })),
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

async fn list_comments(app: &TestApp, token: &str, space_id: &str, page: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments"),
        Some(token),
        None,
    )
    .await
}

async fn add_comment(
    app: &TestApp,
    token: &str,
    space_id: &str,
    page: &str,
    body: serde_json::Value,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments"),
        Some(token),
        Some(&body),
    )
    .await
}

#[tokio::test]
async fn create_list_and_reply_comments() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let member = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let member_token = issue_token(&app, member);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, member, "viewer").await;
    let page = create_page(&app, &owner_token, &space_id, "评审文档").await;
    write_content(&app, &owner_token, &space_id, &page, "# 方案\n\n## 时间安排\n").await;

    let created = add_comment(
        &app,
        &member_token,
        &space_id,
        &page,
        json!({ "body": "这里的排期太紧", "anchor": { "heading": "时间安排" } }),
    )
    .await;
    let body = created.expect(StatusCode::CREATED);
    assert_eq!(body["body"], "这里的排期太紧");
    assert_eq!(body["anchor"]["heading"], "时间安排");
    assert!(body["resolvedAt"].is_null());
    let root_id = body["id"].as_str().unwrap().to_string();

    // 回复
    let reply = add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "收到，会调整", "parentId": root_id }),
    )
    .await;
    let reply_body = reply.expect(StatusCode::CREATED);
    assert_eq!(reply_body["parentId"], root_id);

    let listed = list_comments(&app, &member_token, &space_id, &page).await;
    let listed = listed.expect(StatusCode::OK);
    let rows = listed.as_array().unwrap();
    assert_eq!(rows.len(), 2, "评论与回复各一条");
    assert_eq!(rows[0]["body"], "这里的排期太紧");
    assert_eq!(rows[1]["parentId"], root_id);
}

#[tokio::test]
async fn resolve_and_delete_permissions() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, viewer, "viewer").await;
    let page = create_page(&app, &owner_token, &space_id, "权限文档").await;

    let created = add_comment(
        &app,
        &viewer_token,
        &space_id,
        &page,
        json!({ "body": "看下这段" }),
    )
    .await;
    let comment_id = created.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 作者解决
    let resolved = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{comment_id}"),
        Some(&viewer_token),
        Some(&json!({ "resolved": true })),
    )
    .await;
    let body = resolved.expect(StatusCode::OK);
    assert!(!body["resolvedAt"].is_null());

    // viewer 不能解决别人的评论
    let other = add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "作者评论" }),
    )
    .await;
    let other_id = other.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let denied = request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{other_id}"),
        Some(&viewer_token),
        Some(&json!({ "resolved": true })),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    // 编辑者可以解决 viewer 的评论；viewer 不能删作者评论；作者可删自己的
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{comment_id}"),
        Some(&owner_token),
        Some(&json!({ "resolved": false })),
    )
    .await
    .expect(StatusCode::OK);

    let denied = request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{other_id}"),
        Some(&viewer_token),
        None,
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{comment_id}"),
        Some(&viewer_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let listed = list_comments(&app, &owner_token, &space_id, &page).await;
    let listed = listed.expect(StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 1, "只删自己的评论");
}

#[tokio::test]
async fn delete_parent_removes_replies_and_validates() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    let page = create_page(&app, &owner_token, &space_id, "级联文档").await;

    let root = add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "根评论" }),
    )
    .await;
    let root_id = root.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "回复", "parentId": root_id }),
    )
    .await
    .expect(StatusCode::CREATED);

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/comments/{root_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let listed = list_comments(&app, &owner_token, &space_id, &page).await;
    assert!(listed.expect(StatusCode::OK).as_array().unwrap().is_empty());

    // 空正文
    let empty = add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "   " }),
    )
    .await;
    assert_eq!(
        empty.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );

    // 非成员
    list_comments(&app, &outsider_token, &space_id, &page)
        .await
        .expect(StatusCode::FORBIDDEN);
    add_comment(
        &app,
        &outsider_token,
        &space_id,
        &page,
        json!({ "body": "外人评论" }),
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 父评论必须是同一文档
    let other_page = create_page(&app, &owner_token, &space_id, "另一篇").await;
    let other_comment = add_comment(
        &app,
        &owner_token,
        &space_id,
        &other_page,
        json!({ "body": "别处评论" }),
    )
    .await;
    let other_id = other_comment.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let bad_parent = add_comment(
        &app,
        &owner_token,
        &space_id,
        &page,
        json!({ "body": "跨文档回复", "parentId": other_id }),
    )
    .await;
    bad_parent.expect(StatusCode::UNPROCESSABLE_ENTITY);
}
