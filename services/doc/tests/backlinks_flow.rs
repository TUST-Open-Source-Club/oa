//! P2 反向链接失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 正文里的 `[[标题]]` 视为内链；保存时解析并记录（同空间、同标题的页面）；
//! - `GET /spaces/{id}/nodes/{nodeId}/backlinks` 返回引用该文档的页面列表
//!   （`{ id, title, spaceId, updatedAt }`）；
//! - 未匹配的标题忽略；被引用文档删除后不再出现在别人的反向链接里；
//! - 非成员 403。

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
        Some(&json!({ "name": "内链库", "type": "team" })),
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

async fn write(app: &TestApp, token: &str, space_id: &str, node: &str, md: &str, base: i64) {
    request(
        &app.app,
        "PUT",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node}/content"),
        Some(token),
        Some(&json!({ "contentMd": md, "baseVersion": base })),
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

async fn backlinks(app: &TestApp, token: &str, space_id: &str, node: &str) -> TestResponse {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node}/backlinks"),
        Some(token),
        None,
    )
    .await
}

#[tokio::test]
async fn wiki_links_produce_backlinks() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let space_id = create_space(&app, &owner_token).await;

    let target = create_page(&app, &owner_token, &space_id, "招新方案").await;
    let source = create_page(&app, &owner_token, &space_id, "会议纪要").await;
    write(
        &app,
        &owner_token,
        &space_id,
        &source,
        "讨论见 [[招新方案]]，另见 [[不存在的页面]]。",
        1,
    )
    .await;

    let links = backlinks(&app, &owner_token, &space_id, &target).await;
    let links = links.expect(StatusCode::OK);
    let rows = links.as_array().unwrap();
    assert_eq!(rows.len(), 1, "只记录能解析的链接：{links}");
    assert_eq!(rows[0]["id"], source);
    assert_eq!(rows[0]["title"], "会议纪要");
    assert_eq!(rows[0]["spaceId"], space_id);

    // 重复保存不会产生重复行
    write(
        &app,
        &owner_token,
        &space_id,
        &source,
        "再次提到 [[招新方案]]。",
        2,
    )
    .await;
    let links = backlinks(&app, &owner_token, &space_id, &target).await;
    assert_eq!(links.expect(StatusCode::OK).as_array().unwrap().len(), 1);

    // 引用被删除 → 反向链接消失
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{source}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);
    let links = backlinks(&app, &owner_token, &space_id, &target).await;
    assert!(links.expect(StatusCode::OK).as_array().unwrap().is_empty());
}

#[tokio::test]
async fn backlinks_require_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    let page = create_page(&app, &owner_token, &space_id, "文档").await;

    backlinks(&app, &outsider_token, &space_id, &page)
        .await
        .expect(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn template_import_also_indexes_links() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, viewer, "viewer").await;

    let target = create_page(&app, &owner_token, &space_id, "规范").await;
    let imported = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/import"),
        Some(&owner_token),
        Some(&json!({ "title": "导入文档", "contentMd": "参考 [[规范]] 执行。" })),
    )
    .await;
    let imported = imported.expect(StatusCode::CREATED);
    let source = imported["id"].as_str().unwrap().to_string();

    let links = backlinks(&app, &viewer_token, &space_id, &target).await;
    let links = links.expect(StatusCode::OK);
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(links[0]["id"], source);
}
