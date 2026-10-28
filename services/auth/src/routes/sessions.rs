//! 登录设备（会话）管理：列出活跃设备、登出指定设备。

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use club_auth_sdk::AuthUser;
use club_common::AppError;

use crate::crypto;
use crate::repo;
use crate::state::SharedState;

/// 读取 BFF 注入的当前刷新令牌哈希对应的 family（没有则 None）。
async fn current_family(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<Option<Uuid>, AppError> {
    let Some(refresh) = headers
        .get("x-refresh-token")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
    else {
        return Ok(None);
    };
    let hash = crypto::hash_token(&refresh);
    Ok(repo::find_refresh_by_hash(&state.db, &hash)
        .await?
        .map(|row| row.family_id))
}

/// `GET /sessions`：我的活跃登录设备。
pub async fn list_sessions(
    State(state): State<SharedState>,
    auth: AuthUser,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let user_id = auth
        .claims()
        .sub
        .parse::<Uuid>()
        .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "访问令牌无效"))?;
    let current = current_family(&state, &headers).await?;
    let rows = repo::list_sessions(&state.db, user_id, state.now()).await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "familyId": row.family_id,
                "deviceId": row.device_id,
                "userAgent": row.user_agent,
                "ip": row.ip,
                "createdAt": row.created_at,
                "expiresAt": row.expires_at,
                "current": current == Some(row.family_id),
            })
        })
        .collect();
    Ok(Json(json!(items)))
}

/// `DELETE /sessions/{family_id}`：登出指定设备（吊销整条令牌链）。
pub async fn revoke_session(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(family_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = auth
        .claims()
        .sub
        .parse::<Uuid>()
        .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "访问令牌无效"))?;
    let revoked = repo::revoke_family_for_user(&state.db, user_id, family_id, state.now()).await?;
    if !revoked {
        return Err(AppError::not_found(
            "AUTH_SESSION_NOT_FOUND",
            "设备不存在或已登出",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
