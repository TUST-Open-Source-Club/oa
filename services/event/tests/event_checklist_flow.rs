//! P0 活动筹备清单关联失败用例（TDD 红阶段）。
//!
//! 约定：
//! - 活动 DTO 返回 `taskProjectId` / `taskBoardId` / `checklistTemplate`；
//! - `PUT /events/{id}/checklist` `{ projectId, boardId, templateKey }` 回写筹备项目/看板；
//! - 只有活动管理员可写；重复调用幂等（覆盖为同一值不报错）。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use uuid::Uuid;

async fn create_event(app: &TestApp, token: &str, slug: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/event/events",
        Some(token),
        Some(&json!({ "slug": slug, "title": "招新宣讲", "capacity": 50 })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn attach_checklist_and_read_back() {
    let app = spawn().await;
    let admin = Uuid::now_v7();
    let token = issue_token(&app, admin);
    let event_id = create_event(&app, &token, "recruit-talk").await;
    let project_id = Uuid::now_v7();
    let board_id = Uuid::now_v7();

    let updated = request(
        &app.app,
        "PUT",
        &format!("/api/v1/event/events/{event_id}/checklist"),
        Some(&token),
        Some(&json!({ "projectId": project_id, "boardId": board_id, "templateKey": "recruit" })),
    )
    .await;
    let updated = updated.expect(StatusCode::OK);
    assert_eq!(updated["taskProjectId"], project_id.to_string());
    assert_eq!(updated["taskBoardId"], board_id.to_string());
    assert_eq!(updated["checklistTemplate"], "recruit");

    // 幂等重复
    let again = request(
        &app.app,
        "PUT",
        &format!("/api/v1/event/events/{event_id}/checklist"),
        Some(&token),
        Some(&json!({ "projectId": project_id, "boardId": board_id, "templateKey": "recruit" })),
    )
    .await;
    again.expect(StatusCode::OK);

    // 列表也带
    let list = request(&app.app, "GET", "/api/v1/event/events", Some(&token), None).await;
    let list = list.expect(StatusCode::OK);
    let row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == event_id)
        .unwrap();
    assert_eq!(row["taskBoardId"], board_id.to_string());
}

#[tokio::test]
async fn checklist_requires_event_admin() {
    let app = spawn().await;
    let admin = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let admin_token = issue_token(&app, admin);
    let outsider_token = issue_token(&app, outsider);
    let event_id = create_event(&app, &admin_token, "lecture").await;

    let denied = request(
        &app.app,
        "PUT",
        &format!("/api/v1/event/events/{event_id}/checklist"),
        Some(&outsider_token),
        Some(&json!({ "projectId": Uuid::now_v7(), "boardId": Uuid::now_v7() })),
    )
    .await;
    denied.expect(StatusCode::FORBIDDEN);
}
