//! P0 目录树操作（重命名/移动/排序/软删/恢复）失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `PATCH /spaces/{id}/nodes/{nodeId}`：`{ title?, parentId?, position? }`，返回更新后的节点；
//! - `DELETE /spaces/{id}/nodes/{nodeId}`：软删该节点及整棵子树；
//! - `POST /spaces/{id}/nodes/{nodeId}/restore`：恢复整棵子树；
//! - 不允许移动到自己的子孙（422 `DOC_TREE_CYCLE`），父节点必须是文件夹（422 `DOC_VALIDATION`）；
//! - 非空间成员 403，跨空间节点 404。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_space(app: &TestApp, token: &str) -> String {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/doc/spaces",
        Some(token),
        Some(&json!({ "name": "树操作库", "type": "team" })),
    )
    .await;
    response.expect(StatusCode::CREATED)["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_node(
    app: &TestApp,
    token: &str,
    space_id: &str,
    kind: &str,
    title: &str,
    parent_id: Option<&str>,
) -> Value {
    let mut body = json!({ "kind": kind, "title": title });
    if let Some(parent) = parent_id {
        body["parentId"] = json!(parent);
    }
    let response = request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes"),
        Some(token),
        Some(&body),
    )
    .await;
    response.expect(StatusCode::CREATED)
}

async fn patch_node(
    app: &TestApp,
    token: &str,
    space_id: &str,
    node_id: &str,
    body: Value,
) -> TestResponse {
    request(
        &app.app,
        "PATCH",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{node_id}"),
        Some(token),
        Some(&body),
    )
    .await
}

async fn tree(app: &TestApp, token: &str, space_id: &str) -> Value {
    request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/tree"),
        Some(token),
        None,
    )
    .await
    .expect(StatusCode::OK)
}

/// 在树里找节点（递归），返回 (节点, 父节点 id)。
fn find_in_tree<'a>(nodes: &'a Value, id: &str) -> Option<(&'a Value, Option<&'a str>)> {
    for item in nodes.as_array()? {
        if item["id"] == id {
            return Some((item, None));
        }
        if let Some((hit, _)) = find_in_tree(&item["children"], id) {
            return Some((hit, item["id"].as_str()));
        }
    }
    None
}

#[tokio::test]
async fn rename_move_and_reorder() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;

    let folder_a = create_node(&app, &token, &space_id, "folder", "目录 A", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let folder_b = create_node(&app, &token, &space_id, "folder", "目录 B", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let page = create_node(&app, &token, &space_id, "page", "原页面", Some(&folder_a)).await;
    let page_id = page["id"].as_str().unwrap().to_string();

    // 重命名
    let renamed = patch_node(
        &app,
        &token,
        &space_id,
        &page_id,
        json!({ "title": "新标题" }),
    )
    .await;
    let body = renamed.expect(StatusCode::OK);
    assert_eq!(body["title"], "新标题");
    assert_eq!(body["parentId"], folder_a);

    let detail = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(detail.expect(StatusCode::OK)["title"], "新标题");

    // 移动：A → B
    let moved = patch_node(
        &app,
        &token,
        &space_id,
        &page_id,
        json!({ "parentId": folder_b }),
    )
    .await;
    assert_eq!(moved.expect(StatusCode::OK)["parentId"], folder_b);
    let snapshot = tree(&app, &token, &space_id).await;
    let (_, parent) = find_in_tree(&snapshot, &page_id).expect("页面在树里");
    assert_eq!(parent, Some(folder_b.as_str()));

    // 排序：再建一页，把 page 置顶
    let second = create_node(&app, &token, &space_id, "page", "第二页", Some(&folder_b)).await;
    let second_id = second["id"].as_str().unwrap().to_string();
    let reordered = patch_node(
        &app,
        &token,
        &space_id,
        &page_id,
        json!({ "position": 0 }),
    )
    .await;
    assert_eq!(reordered.expect(StatusCode::OK)["position"], 0);
    let snapshot = tree(&app, &token, &space_id).await;
    let (folder, _) = find_in_tree(&snapshot, &folder_b).expect("目录 B");
    let order: Vec<&str> = folder["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        vec![page_id.as_str(), second_id.as_str()],
        "position 小的排在前面"
    );
}

#[tokio::test]
async fn move_rejects_cycles_and_invalid_parents() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;

    let outer = create_node(&app, &token, &space_id, "folder", "外层", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let inner = create_node(&app, &token, &space_id, "folder", "内层", Some(&outer)).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let page = create_node(&app, &token, &space_id, "page", "页面", Some(&outer)).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 移到自己子孙下 → 循环
    let cyclic = patch_node(
        &app,
        &token,
        &space_id,
        &outer,
        json!({ "parentId": inner }),
    )
    .await;
    let body = cyclic.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "DOC_TREE_CYCLE");

    // 父节点必须是文件夹
    let bad_parent = patch_node(
        &app,
        &token,
        &space_id,
        &inner,
        json!({ "parentId": page }),
    )
    .await;
    let body = bad_parent.expect(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "DOC_VALIDATION");

    // 跨空间父节点 → 404
    let other_space = create_space(&app, &token).await;
    let foreign = create_node(&app, &token, &other_space, "folder", "别处", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let foreign_move = patch_node(
        &app,
        &token,
        &space_id,
        &page,
        json!({ "parentId": foreign }),
    )
    .await;
    foreign_move.expect(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_hides_subtree_and_restore_brings_it_back() {
    let app = spawn().await;
    let user = Uuid::now_v7();
    let token = issue_token(&app, user);
    let space_id = create_space(&app, &token).await;

    let folder = create_node(&app, &token, &space_id, "folder", "待删目录", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let page = create_node(&app, &token, &space_id, "page", "里面的页面", Some(&folder)).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{folder}"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::NO_CONTENT);

    // 树里看不到，节点也读不到
    let snapshot = tree(&app, &token, &space_id).await;
    assert!(find_in_tree(&snapshot, &folder).is_none());
    let hidden = request(
        &app.app,
        "GET",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}"),
        Some(&token),
        None,
    )
    .await;
    hidden.expect(StatusCode::NOT_FOUND);

    // 恢复：整棵子树回来
    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{folder}/restore"),
        Some(&token),
        None,
    )
    .await
    .expect(StatusCode::OK);
    let snapshot = tree(&app, &token, &space_id).await;
    assert!(find_in_tree(&snapshot, &folder).is_some());
    assert!(find_in_tree(&snapshot, &page).is_some());
}

#[tokio::test]
async fn tree_ops_require_membership() {
    let app = spawn().await;
    let owner = Uuid::now_v7();
    let outsider = Uuid::now_v7();
    let owner_token = issue_token(&app, owner);
    let outsider_token = issue_token(&app, outsider);
    let space_id = create_space(&app, &owner_token).await;
    let page = create_node(&app, &owner_token, &space_id, "page", "页面", None).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    patch_node(
        &app,
        &outsider_token,
        &space_id,
        &page,
        json!({ "title": "篡改" }),
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);

    request(
        &app.app,
        "POST",
        &format!("/api/v1/doc/spaces/{space_id}/nodes/{page}/restore"),
        Some(&outsider_token),
        None,
    )
    .await
    .expect(StatusCode::FORBIDDEN);
}
