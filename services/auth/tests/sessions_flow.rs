//! 登录设备（会话）管理的失败用例（TDD 红阶段）。
//!
//! 约定：
//! - `GET /api/v1/auth/sessions`：列出我的活跃登录设备（按 family 聚合，密钥只返回元信息）；
//!   可选请求头 `x-refresh-token`（BFF 服务端注入）用于标记 `current`；
//! - `DELETE /api/v1/auth/sessions/{familyId}`：登出该设备（吊销整个令牌链），204；
//! - 只能操作自己的设备（否则 404）；未登录 401。

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// 登录并返回 (accessToken, refreshToken)。
async fn login(app: &TestApp, identifier: &str, password: &str, device: &str) -> (String, String) {
    let response = request(
        &app.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(&json!({ "identifier": identifier, "password": password, "deviceId": device })),
    )
    .await;
    let body = response.expect(StatusCode::OK);
    (
        body["accessToken"].as_str().unwrap().to_string(),
        body["refreshToken"].as_str().unwrap().to_string(),
    )
}

/// 列出设备。
async fn list_sessions(app: &TestApp, token: &str, current_refresh: Option<&str>) -> TestResponse {
    match current_refresh {
        Some(refresh) => {
            request_with_headers(
                &app.app,
                "GET",
                "/api/v1/auth/sessions",
                Some(token),
                None,
                &[("x-refresh-token", refresh.to_string())],
            )
            .await
        }
        None => request(&app.app, "GET", "/api/v1/auth/sessions", Some(token), None).await,
    }
}

/// 登出某个 family。
async fn revoke_session(app: &TestApp, token: &str, family_id: &str) -> TestResponse {
    request(
        &app.app,
        "DELETE",
        &format!("/api/v1/auth/sessions/{family_id}"),
        Some(token),
        None,
    )
    .await
}

/// 在列表里按 deviceId 找一条。
fn find_device<'a>(list: &'a Value, device: &str) -> &'a Value {
    list.as_array()
        .expect("设备数组")
        .iter()
        .find(|row| row["deviceId"] == device)
        .unwrap_or_else(|| panic!("找不到设备 {device}: {list}"))
}

#[tokio::test]
async fn lists_devices_and_marks_current() {
    let app = spawn().await;
    seed_active_user(&app, "dev@club.test", "devuser", "Passw0rd!", &["member"]).await;
    let (token, refresh_b) = login(&app, "dev@club.test", "Passw0rd!", "browser-b").await;
    let (_, refresh_a) = login(&app, "dev@club.test", "Passw0rd!", "browser-a").await;

    let response = list_sessions(&app, &token, Some(&refresh_a)).await;
    let body = response.expect(StatusCode::OK);
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 2, "两个设备两条记录：{body}");
    let a = find_device(&body, "browser-a");
    let b = find_device(&body, "browser-b");
    assert_eq!(a["current"], true, "按 x-refresh-token 标记当前设备");
    assert_eq!(b["current"], false);
    assert!(a["familyId"].as_str().is_some());
    assert!(a["createdAt"].as_str().is_some());

    // 不带 refresh 时全部 current=false（BFF 未注入的场景）
    let response = list_sessions(&app, &token, None).await;
    let body = response.expect(StatusCode::OK);
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["current"] == false));

    // 未登录
    request(
        &app.app,
        "GET",
        "/api/v1/auth/sessions",
        None,
        None,
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    let _ = refresh_b;
}

#[tokio::test]
async fn revoke_device_kills_its_refresh_token() {
    let app = spawn().await;
    seed_active_user(&app, "rev@club.test", "revuser", "Passw0rd!", &["member"]).await;
    let (token, _refresh_a) = login(&app, "rev@club.test", "Passw0rd!", "browser-a").await;
    let (_, stale_refresh) = login(&app, "rev@club.test", "Passw0rd!", "browser-b").await;

    let body = list_sessions(&app, &token, None).await.expect(StatusCode::OK);
    let stale_family = find_device(&body, "browser-b")["familyId"]
        .as_str()
        .unwrap()
        .to_string();

    revoke_session(&app, &token, &stale_family)
        .await
        .expect(StatusCode::NO_CONTENT);

    // 被登出的设备无法再刷新
    request(
        &app.app,
        "POST",
        "/api/v1/auth/refresh",
        None,
        Some(&json!({ "refreshToken": stale_refresh })),
    )
    .await
    .expect(StatusCode::UNAUTHORIZED);

    // 列表只剩另一台
    let body = list_sessions(&app, &token, None).await.expect(StatusCode::OK);
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["deviceId"], "browser-a");

    // 重复登出：已不存在 → 404
    revoke_session(&app, &token, &stale_family)
        .await
        .expect(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cannot_revoke_other_users_device() {
    let app = spawn().await;
    seed_active_user(&app, "owner2@club.test", "owner2", "Passw0rd!", &["member"]).await;
    seed_active_user(&app, "other2@club.test", "other2", "Passw0rd!", &["member"]).await;
    let (owner_token, _) = login(&app, "owner2@club.test", "Passw0rd!", "owner-pc").await;
    let (other_token, _) = login(&app, "other2@club.test", "Passw0rd!", "other-pc").await;

    let body = list_sessions(&app, &other_token, None)
        .await
        .expect(StatusCode::OK);
    let family = body.as_array().unwrap()[0]["familyId"]
        .as_str()
        .unwrap()
        .to_string();

    revoke_session(&app, &owner_token, &family)
        .await
        .expect(StatusCode::NOT_FOUND);

    // 对方设备仍在
    let body = list_sessions(&app, &other_token, None)
        .await
        .expect(StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);

    // 非法 family id → 400/404
    revoke_session(&app, &owner_token, &Uuid::now_v7().to_string())
        .await
        .expect(StatusCode::NOT_FOUND);
}
