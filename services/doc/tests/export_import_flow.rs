//! P0 文档导出/导入失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /spaces/{id}/nodes/{nodeId}/export?format=md|html`：成员可导出单篇页面（附件下载）；
//!   html 为渲染后的 HTML 片段（标题、加粗、表格等）；
//! - `POST /spaces/{id}/import`：编辑者导入 Markdown（`{ title?, contentMd, parentId? }`），
//!   返回新建页面；缺标题时用首个 `# 标题` 或“未命名文档”；
//! - 文件夹不可导出（422）；viewer 不可导入（403）；空正文 422。

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

struct TextResponse {
    status: StatusCode,
    content_type: Option<String>,
    disposition: Option<String>,
    body: String,
}

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

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "导出库", "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_node(app: &TestApp, token: &str, space_id: &str, kind: &str, title: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&json!({ "kind": kind, "title": title })),
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
    use sea_orm::{ActiveModelTrait, Set};
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

#[tokio::test]
async fn export_markdown_and_html() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;
    let page = create_node(&app, &token, &space_id, "page", "导出页面").await;
    let md = "# 标题\n\n这是 **加粗** 正文。\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
    write_content(&app, &token, &space_id, &page, md).await;

    let exported = get_text(
        &app,
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/export?format=md"),
        &token,
    )
    .await;
    assert_eq!(exported.status, StatusCode::OK);
    assert!(exported
        .content_type
        .as_deref()
        .unwrap_or_default()
        .starts_with("text/markdown"));
    assert!(exported
        .disposition
        .as_deref()
        .unwrap_or_default()
        .contains("attachment"));
    assert_eq!(exported.body, md);

    let html = get_text(
        &app,
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/export?format=html"),
        &token,
    )
    .await;
    assert_eq!(html.status, StatusCode::OK);
    assert!(html
        .content_type
        .as_deref()
        .unwrap_or_default()
        .starts_with("text/html"));
    assert!(html.body.contains("<h1>标题</h1>"), "{}", html.body);
    assert!(html.body.contains("<strong>加粗</strong>"), "{}", html.body);
    assert!(html.body.contains("<table>"), "{}", html.body);
}

#[tokio::test]
async fn export_requires_membership_and_page_kind() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    let folder = create_node(&app, &owner_token, &space_id, "folder", "目录").await;
    let page = create_node(&app, &owner_token, &space_id, "page", "页面").await;
    write_content(&app, &owner_token, &space_id, &page, "正文").await;

    let denied = get_text(
        &app,
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/export?format=md"),
        &outsider_token,
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    let folder_export = get_text(
        &app,
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{folder}/export?format=md"),
        &owner_token,
    )
    .await;
    assert_eq!(folder_export.status, StatusCode::UNPROCESSABLE_ENTITY);

    let bad_format = get_text(
        &app,
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/export?format=pdf"),
        &owner_token,
    )
    .await;
    assert_eq!(bad_format.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn import_markdown_creates_page() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;
    let folder = create_node(&app, &token, &space_id, "folder", "导入目录").await;

    let imported = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/import"),
        Some(&token),
        Some(&json!({
            "title": "导入的文档",
            "contentMd": "# 导入的文档\n\n正文内容",
            "parentId": folder,
        })),
    )
    .await;
    let body = imported.expect(StatusCode::CREATED);
    let node_id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["title"], "导入的文档");
    assert_eq!(body["parentId"], folder);

    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node_id}"),
        Some(&token),
        None,
    )
    .await;
    let detail = detail.expect(StatusCode::OK);
    assert!(detail["contentMd"].as_str().unwrap().contains("正文内容"));

    // 不给标题时用首个一级标题
    let inferred = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/import"),
        Some(&token),
        Some(&json!({ "contentMd": "# 自动标题\n\n内容" })),
    )
    .await;
    let body = inferred.expect(StatusCode::CREATED);
    assert_eq!(body["title"], "自动标题");
}

#[tokio::test]
async fn import_validates_role_and_payload() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token).await;
    add_member(&app, &space_id, viewer, "viewer").await;

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/import"),
        Some(&viewer_token),
        Some(&json!({ "title": "x", "contentMd": "正文" })),
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    let empty = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/import"),
        Some(&owner_token),
        Some(&json!({ "title": "空文档", "contentMd": "   " })),
    )
    .await;
    let body = empty.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "DOC_VALIDATION");
}
