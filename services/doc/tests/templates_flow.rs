//! P2 文档模板失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET/POST /spaces/{id}/templates`：列出/新建模板（编辑者）；
//! - `DELETE /templates/{id}`：删除模板（编辑者，且属于自己所在空间）；
//! - `POST /spaces/{id}/nodes/from-template`：`{ templateId, title?, parentId? }` 用模板建页；
//! - viewer 只能看模板不能建/删；跨空间模板 404；空名称/正文 422。

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

async fn create_template(
    app: &TestApp,
    token: &str,
    space_id: &str,
    body: serde_json::Value,
) -> TestResponse {
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/templates"),
        Some(token),
        Some(&body),
    )
    .await
}

#[tokio::test]
async fn create_list_and_use_template() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token, "模板库").await;
    add_member(&app, &space_id, viewer, "viewer").await;

    let created = create_template(
        &app,
        &owner_token,
        &space_id,
        json!({ "name": "周报模板", "contentMd": "# 本周进展\n\n## 下周计划\n" }),
    )
    .await;
    let template = created.expect(StatusCode::CREATED);
    assert_eq!(template["name"], "周报模板");
    let template_id = template["id"].as_str().unwrap().to_string();

    // viewer 可列出
    let listed = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/templates"),
        Some(&viewer_token),
        None,
    )
    .await;
    let listed = listed.expect(StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["name"], "周报模板");

    // 用模板建页
    let page = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/from-template"),
        Some(&owner_token),
        Some(&json!({ "templateId": template_id, "title": "第 12 周周报" })),
    )
    .await;
    let page = page.expect(StatusCode::CREATED);
    assert_eq!(page["title"], "第 12 周周报");

    let detail = request(
        &app.app,
        "GET",
        &format!(
            "/api/v1/doc/spaces/{space_id}/nodes/{}",
            page["id"].as_str().unwrap()
        ),
        Some(&owner_token),
        None,
    )
    .await;
    let detail = detail.expect(StatusCode::OK);
    assert!(detail["contentMd"].as_str().unwrap().contains("本周进展"));
}

#[tokio::test]
async fn template_permissions_and_validation() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let viewer = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let viewer_token = issue_token(&app, viewer);
    let space_id = create_space(&app, &owner_token, "权限模板库").await;
    add_member(&app, &space_id, viewer, "viewer").await;

    // viewer 不能建模板
    create_template(
        &app,
        &viewer_token,
        &space_id,
        json!({ "name": "x", "contentMd": "y" }),
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    // 空名称 / 空正文
    let empty_name = create_template(
        &app,
        &owner_token,
        &space_id,
        json!({ "name": "  ", "contentMd": "正文" }),
    )
    .await;
    assert_eq!(
        empty_name.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );
    let empty_body = create_template(
        &app,
        &owner_token,
        &space_id,
        json!({ "name": "模板", "contentMd": "   " }),
    )
    .await;
    assert_eq!(
        empty_body.expect(StatusCode::UNPROCESSABLE_ENTITY)["code"],
        "DOC_VALIDATION"
    );

    // 跨空间模板 404
    let template = create_template(
        &app,
        &owner_token,
        &space_id,
        json!({ "name": "模板", "contentMd": "内容" }),
    )
    .await
    .expect(StatusCode::CREATED);
    let template_id = template["id"].as_str().unwrap().to_string();
    let other_space = create_space(&app, &owner_token, "另一个库").await;
    let missing = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{other_space}/nodes/from-template"),
        Some(&owner_token),
        Some(&json!({ "templateId": template_id, "title": "跨库" })),
    )
    .await;
    missing.expect(StatusCode::NOT_FOUND);

    // 删除
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/templates/{template_id}"),
        Some(&viewer_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/templates/{template_id}"),
        Some(&owner_token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    let listed = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/templates"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(listed
        .expect(StatusCode::OK)
        .as_array()
        .unwrap()
        .is_empty());
}
