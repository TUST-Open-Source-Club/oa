//! HTTP 路由。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use club_auth_sdk::AuthUser;
use club_common::{AppError, FieldError};

use crate::entity::{node, space};
use crate::repo;
use crate::state::SharedState;

/// 存活检查。
pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// 就绪检查。
pub async fn readyz(State(state): State<SharedState>) -> Json<Value> {
    match state.db.ping().await {
        Ok(_) => Json(json!({ "status": "ready", "database": "ok" })),
        Err(err) => {
            tracing::error!(error = %err, "数据库就绪检查失败");
            Json(json!({ "status": "degraded", "database": "error" }))
        }
    }
}

/// 用户 ID。
fn user_id_of(auth: &AuthUser) -> Result<Uuid, AppError> {
    auth.claims()
        .sub
        .parse()
        .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "访问令牌无效"))
}

/// 节点 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDto {
    /// ID。
    pub id: String,
    /// 父节点。
    pub parent_id: Option<String>,
    /// folder / page。
    pub kind: String,
    /// 标题。
    pub title: String,
    /// 排序。
    pub position: i64,
    /// 版本。
    pub version: i64,
    /// 更新时间。
    pub updated_at: DateTime<FixedOffset>,
}

impl From<&node::Model> for NodeDto {
    fn from(model: &node::Model) -> Self {
        Self {
            id: model.id.to_string(),
            parent_id: model.parent_id.map(|id| id.to_string()),
            kind: model.kind.clone(),
            title: model.title.clone(),
            position: model.position,
            version: model.version,
            updated_at: model.updated_at,
        }
    }
}

/// 创建空间请求。
#[derive(Debug, Deserialize)]
pub struct CreateSpaceRequest {
    /// 名称。
    pub name: String,
    /// team/personal/public。
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `POST /spaces`。
pub async fn create_space(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(input): Json<CreateSpaceRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let user_id = user_id_of(&auth)?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "空间名称需为 1 ~ 64 字符",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let kind = input.kind.unwrap_or_else(|| space::TYPE_TEAM.to_string());
    if ![space::TYPE_TEAM, space::TYPE_PERSONAL, space::TYPE_PUBLIC].contains(&kind.as_str()) {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "空间类型不合法",
            vec![FieldError::new("type", "仅支持 team/personal/public")],
        ));
    }
    let model = repo::create_space(&state.db, user_id, name, &kind, state.now()).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": model.id, "name": model.name, "type": model.r#type })),
    ))
}

/// `GET /spaces/{id}/nodes/{node_id}/changes`：文档操作记录。
pub async fn list_node_changes(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<club_bus::audit::ChangeEntry>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, space_id, user_id).await?;
    load_node(&state, space_id, node_id).await?;
    let items = club_bus::audit::list_for(&state.db, "doc_node", &node_id.to_string(), 50)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(items))
}

/// `POST /changes/{id}/undo`：撤销一次正文变更（以新版本写回旧内容）。
pub async fn undo_change(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(change_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let entry = club_bus::audit::find(&state.db, change_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("DOC_CHANGE_NOT_FOUND", "变更记录不存在"))?;
    if entry.entity != "doc_node" {
        return Err(AppError::unprocessable(
            "DOC_UNDO_UNSUPPORTED",
            "仅支持撤销文档正文变更",
            vec![],
        ));
    }
    let before = entry
        .before
        .clone()
        .ok_or_else(|| AppError::unprocessable("DOC_UNDO_UNSUPPORTED", "该记录不可撤销", vec![]))?;
    let node_id: Uuid = entry.entity_id.parse().map_err(AppError::internal)?;
    let model = repo::find_node(&state.db, node_id)
        .await?
        .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "节点不存在"))?;
    repo::ensure_editor(&state.db, model.space_id, user_id).await?;
    let content = before
        .get("contentMd")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let now = state.now();
    let updated =
        repo::update_content(&state.db, &model, model.version, content, user_id, now).await?;
    let _ = club_bus::audit::record(
        &state.db,
        "doc_node",
        &updated.id.to_string(),
        "undo",
        Some(json!({ "contentMd": model.content_md, "version": model.version })),
        Some(json!({ "contentMd": updated.content_md, "version": updated.version })),
        Some(user_id),
        now,
    )
    .await;
    Ok(Json(
        json!({ "version": updated.version, "updatedAt": updated.updated_at }),
    ))
}

/// `DELETE /spaces/{id}`：删除知识库（仅空间管理员）。
pub async fn delete_space(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(space_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let member = repo::ensure_member(&state.db, space_id, user_id).await?;
    if member.role != "admin" {
        return Err(AppError::forbidden(
            "DOC_FORBIDDEN",
            "仅空间管理员可删除知识库",
        ));
    }
    repo::delete_space(&state.db, space_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /spaces`。
pub async fn list_spaces(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let spaces = repo::list_spaces(&state.db, user_id).await?;
    Ok(Json(json!(spaces
        .iter()
        .map(|space| json!({ "id": space.id, "name": space.name, "type": space.r#type }))
        .collect::<Vec<_>>())))
}

/// `GET /spaces/{id}/tree`：嵌套树。
pub async fn space_tree(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(space_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let member = repo::ensure_member(&state.db, space_id, user_id).await?;
    let mut nodes = repo::list_nodes(&state.db, space_id).await?;
    if member.role == "viewer" {
        nodes.retain(|node| node.visibility != "private");
    }
    Ok(Json(build_tree(&nodes, None)))
}

/// 递归构建树。
fn build_tree(nodes: &[node::Model], parent: Option<Uuid>) -> Value {
    let children: Vec<Value> = nodes
        .iter()
        .filter(|item| item.parent_id == parent)
        .map(|item| {
            let mut entry = serde_json::to_value(NodeDto::from(item)).unwrap_or(Value::Null);
            entry["children"] = build_tree(nodes, Some(item.id));
            entry
        })
        .collect();
    Value::Array(children)
}

/// 创建节点请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeRequest {
    /// folder / page。
    pub kind: String,
    /// 标题。
    pub title: String,
    /// 父节点。
    pub parent_id: Option<Uuid>,
}

/// `POST /spaces/{id}/nodes`。
pub async fn create_node(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(space_id): Path<Uuid>,
    Json(input): Json<CreateNodeRequest>,
) -> Result<(StatusCode, Json<NodeDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    if ![node::KIND_FOLDER, node::KIND_PAGE].contains(&input.kind.as_str()) {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "节点类型不合法",
            vec![FieldError::new("kind", "仅支持 folder/page")],
        ));
    }
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 255 {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "标题需为 1 ~ 255 字符",
            vec![FieldError::new("title", "非法")],
        ));
    }
    if let Some(parent_id) = input.parent_id {
        let parent = repo::find_node(&state.db, parent_id)
            .await?
            .filter(|item| item.space_id == space_id && item.deleted_at.is_none())
            .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "父节点不存在"))?;
        if parent.kind != node::KIND_FOLDER {
            return Err(AppError::unprocessable(
                "DOC_VALIDATION",
                "父节点必须是文件夹",
                vec![FieldError::new("parentId", "必须是文件夹")],
            ));
        }
    }
    let model = repo::create_node(
        &state.db,
        space_id,
        input.parent_id,
        &input.kind,
        title,
        user_id,
        state.now(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(NodeDto::from(&model))))
}

/// `GET /spaces/{id}/nodes/{node_id}`：节点详情（含正文）。
pub async fn get_node(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let member = repo::ensure_member(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    if model.visibility == "private" && member.role == "viewer" {
        return Err(AppError::forbidden("DOC_FORBIDDEN", "该文档仅编辑者可见"));
    }
    Ok(Json(json!({
        "id": model.id,
        "kind": model.kind,
        "title": model.title,
        "version": model.version,
        "contentMd": model.content_md,
        "visibility": model.visibility,
        "updatedAt": model.updated_at
    })))
}

/// 分享设置请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRequest {
    /// 访问密码（null 表示无密码）。
    pub password: Option<String>,
    /// 过期时间（null 表示长期）。
    pub expires_at: Option<DateTime<chrono::FixedOffset>>,
    /// 权限（当前仅 read）。
    pub permission: Option<String>,
}

/// 分享密码哈希：sha256(token || password)，token 作为 128 位随机盐。
fn hash_share_password(token: &str, password: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

/// 校验分享密码（常量时间比较）。
fn verify_share_password(stored: &str, token: &str, password: &str) -> bool {
    let expected = hash_share_password(token, password);
    stored.len() == expected.len()
        && stored
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (left, right)| acc | (left ^ right))
            == 0
}

/// `POST /spaces/{id}/nodes/{node_id}/share`：生成或更新分享链接（仅编辑者）。
pub async fn create_share(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ShareRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    if model.kind != node::KIND_PAGE {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "仅页面支持分享",
            vec![FieldError::new("kind", "文件夹不能分享")],
        ));
    }
    let permission = input.permission.as_deref().unwrap_or("read");
    if permission != "read" {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "分享权限不合法",
            vec![FieldError::new("permission", "当前仅支持 read")],
        ));
    }
    let token = model
        .share_token
        .clone()
        .unwrap_or_else(|| club_common::new_id().simple().to_string());
    let password_hash = match input.password.as_deref().map(str::trim) {
        Some(password) if !password.is_empty() => {
            if password.chars().count() > 64 {
                return Err(AppError::unprocessable(
                    "DOC_VALIDATION",
                    "密码过长",
                    vec![FieldError::new("password", "不超过 64 字")],
                ));
            }
            Some(hash_share_password(&token, password))
        }
        _ => None,
    };
    let updated = repo::set_share(
        &state.db,
        &model,
        &token,
        password_hash.as_deref(),
        input
            .expires_at
            .map(|value| value.with_timezone(&chrono::Utc)),
        permission,
        state.now(),
    )
    .await?;
    Ok(Json(json!({
        "token": token,
        "url": format!("/s/doc/{token}"),
        "hasPassword": updated.share_password_hash.is_some(),
        "expiresAt": updated.share_expires_at,
        "permission": updated.share_permission,
    })))
}

/// `GET /spaces/{id}/nodes/{node_id}/share`：查看当前分享设置。
pub async fn get_share(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    let token = model.share_token.clone();
    Ok(Json(json!({
        "token": token,
        "url": token.as_ref().map(|value| format!("/s/doc/{value}")),
        "hasPassword": model.share_password_hash.is_some(),
        "expiresAt": model.share_expires_at,
        "permission": model.share_permission,
    })))
}

/// `DELETE /spaces/{id}/nodes/{node_id}/share`：关闭分享。
pub async fn delete_share(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    repo::clear_share(&state.db, &model, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 公开读取的查询参数。
#[derive(Debug, Deserialize)]
pub struct PublicDocQuery {
    /// 分享密码。
    pub password: Option<String>,
}

/// `GET /public/{token}`：免登录只读访问分享文档。
pub async fn public_doc(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    axum::extract::Query(query): axum::extract::Query<PublicDocQuery>,
) -> Result<Json<Value>, AppError> {
    let model = repo::find_node_by_share_token(&state.db, &token)
        .await?
        .filter(|node| node.deleted_at.is_none())
        .ok_or_else(|| AppError::not_found("DOC_SHARE_NOT_FOUND", "分享不存在或已关闭"))?;
    if let Some(expires_at) = model.share_expires_at {
        if expires_at.with_timezone(&chrono::Utc) <= state.now() {
            return Err(AppError::gone("DOC_SHARE_EXPIRED", "分享链接已过期"));
        }
    }
    if let Some(hash) = model.share_password_hash.as_deref() {
        let Some(password) = query.password.as_deref() else {
            return Err(AppError::unauthorized(
                "DOC_SHARE_PASSWORD_REQUIRED",
                "该分享需要访问密码",
            ));
        };
        if !verify_share_password(hash, &token, password) {
            return Err(AppError::unauthorized(
                "DOC_SHARE_PASSWORD_INVALID",
                "访问密码不正确",
            ));
        }
    }
    let space = crate::entity::space::Entity::find_by_id(model.space_id)
        .one(&state.db)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(json!({
        "title": model.title,
        "contentMd": model.content_md,
        "updatedAt": model.updated_at,
        "spaceName": space.map(|item| item.name),
        "permission": model.share_permission.clone().unwrap_or_else(|| "read".to_string()),
    })))
}

/// 加载并校验节点。
async fn load_node(
    state: &SharedState,
    space_id: Uuid,
    node_id: Uuid,
) -> Result<node::Model, AppError> {
    repo::find_node(&state.db, node_id)
        .await?
        .filter(|item| item.space_id == space_id && item.deleted_at.is_none())
        .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "节点不存在"))
}

/// 三态 JSON（区分「字段缺省」与「显式 null」）。
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 更新节点元信息请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchNodeRequest {
    /// 新标题。
    pub title: Option<String>,
    /// 新父节点（显式 null 表示移到根）。
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<Uuid>>,
    /// 同级排序。
    pub position: Option<i64>,
    /// 可见性：space / private。
    pub visibility: Option<String>,
}

/// `PATCH /spaces/{id}/nodes/{node_id}`：重命名 / 移动 / 排序。
pub async fn patch_node(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<PatchNodeRequest>,
) -> Result<Json<NodeDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;

    let title = match input.title.as_deref() {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.chars().count() > 255 {
                return Err(AppError::unprocessable(
                    "DOC_VALIDATION",
                    "标题需为 1 ~ 255 字符",
                    vec![FieldError::new("title", "非法")],
                ));
            }
            Some(trimmed.to_string())
        }
        None => None,
    };

    if let Some(Some(parent_id)) = input.parent_id {
        let parent = repo::find_node(&state.db, parent_id)
            .await?
            .filter(|item| item.space_id == space_id && item.deleted_at.is_none())
            .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "父节点不存在"))?;
        if parent.kind != node::KIND_FOLDER {
            return Err(AppError::unprocessable(
                "DOC_VALIDATION",
                "父节点必须是文件夹",
                vec![FieldError::new("parentId", "必须是文件夹")],
            ));
        }
        let descendants = repo::descendant_ids(&state.db, space_id, node_id).await?;
        if descendants.contains(&parent_id) {
            return Err(AppError::unprocessable(
                "DOC_TREE_CYCLE",
                "不能把节点移动到自己的子孙下",
                vec![FieldError::new("parentId", "会形成循环")],
            ));
        }
    }

    if let Some(visibility) = input.visibility.as_deref() {
        if !["space", "private"].contains(&visibility) {
            return Err(AppError::unprocessable(
                "DOC_VALIDATION",
                "可见性不合法",
                vec![FieldError::new("visibility", "仅支持 space / private")],
            ));
        }
    }

    let updated = repo::update_node_meta(
        &state.db,
        &model,
        repo::NodeMetaUpdate {
            title: title.as_deref(),
            parent_id: input.parent_id,
            position: input.position,
            visibility: input.visibility.as_deref(),
        },
        user_id,
        state.now(),
    )
    .await?;
    Ok(Json(NodeDto::from(&updated)))
}

/// `DELETE /spaces/{id}/nodes/{node_id}`：软删节点及整棵子树。
pub async fn delete_node(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    load_node(&state, space_id, node_id).await?;
    repo::soft_delete_subtree(&state.db, space_id, node_id, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /spaces/{id}/nodes/{node_id}/restore`：恢复整棵子树。
pub async fn restore_node(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<NodeDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = repo::find_node(&state.db, node_id)
        .await?
        .filter(|item| item.space_id == space_id && item.deleted_at.is_some())
        .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "节点不存在或未删除"))?;
    // 父级仍处于删除状态时不允许恢复，避免挂到已删除目录下
    if let Some(parent_id) = model.parent_id {
        let parent = repo::find_node(&state.db, parent_id).await?;
        if parent.map(|item| item.deleted_at.is_some()).unwrap_or(true) {
            return Err(AppError::unprocessable(
                "DOC_PARENT_DELETED",
                "父节点已删除，请先恢复父节点",
                vec![FieldError::new("parentId", "父节点已删除")],
            ));
        }
    }
    repo::restore_subtree(&state.db, space_id, node_id, state.now()).await?;
    let restored = repo::find_node(&state.db, node_id)
        .await?
        .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "节点不存在"))?;
    Ok(Json(NodeDto::from(&restored)))
}

/// 更新正文请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContentRequest {
    /// Markdown 正文。
    pub content_md: String,
    /// 期望的当前版本（乐观锁）。
    pub base_version: i64,
}

/// `PUT /spaces/{id}/nodes/{node_id}/content`。
pub async fn update_content(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateContentRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    if model.kind != node::KIND_PAGE {
        return Err(AppError::unprocessable(
            "DOC_VALIDATION",
            "仅页面可编辑正文",
            vec![FieldError::new("contentMd", "文件夹无正文")],
        ));
    }
    let updated = repo::update_content(
        &state.db,
        &model,
        input.base_version,
        &input.content_md,
        user_id,
        state.now(),
    )
    .await?;
    let _ = club_bus::audit::record(
        &state.db,
        "doc_node",
        &updated.id.to_string(),
        "update",
        Some(json!({ "contentMd": model.content_md, "version": model.version })),
        Some(json!({ "contentMd": updated.content_md, "version": updated.version })),
        Some(user_id),
        state.now(),
    )
    .await;
    Ok(Json(
        json!({ "version": updated.version, "updatedAt": updated.updated_at }),
    ))
}

/// `GET /spaces/{id}/nodes/{node_id}/versions`。
pub async fn list_versions(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, space_id, user_id).await?;
    load_node(&state, space_id, node_id).await?;
    let versions = repo::list_versions(&state.db, node_id).await?;
    Ok(Json(json!(versions
        .iter()
        .map(|item| json!({
            "version": item.version,
            "authorId": item.author_id,
            "createdAt": item.created_at
        }))
        .collect::<Vec<_>>())))
}

/// `POST /spaces/{id}/nodes/{node_id}/versions/{version}/restore`：回滚到历史版本。
pub async fn restore_version(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((space_id, node_id, target_version)): Path<(Uuid, Uuid, i64)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_editor(&state.db, space_id, user_id).await?;
    let model = load_node(&state, space_id, node_id).await?;
    let target = repo::find_version(&state.db, node_id, target_version)
        .await?
        .ok_or_else(|| AppError::not_found("DOC_VERSION_NOT_FOUND", "历史版本不存在"))?;
    let updated = repo::update_content(
        &state.db,
        &model,
        model.version,
        &target.content_md,
        user_id,
        state.now(),
    )
    .await?;
    Ok(Json(json!({ "version": updated.version })))
}

/// `/api/v1/doc` 路由。
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/spaces", get(list_spaces).post(create_space))
        .route("/spaces/{id}", axum::routing::delete(delete_space))
        .route("/spaces/{id}/tree", get(space_tree))
        .route("/spaces/{id}/nodes", post(create_node))
        .route(
            "/spaces/{id}/nodes/{node_id}",
            get(get_node).patch(patch_node).delete(delete_node),
        )
        .route("/spaces/{id}/nodes/{node_id}/restore", post(restore_node))
        .route(
            "/spaces/{id}/nodes/{node_id}/content",
            axum::routing::put(update_content),
        )
        .route(
            "/spaces/{id}/nodes/{node_id}/changes",
            get(list_node_changes),
        )
        .route("/changes/{id}/undo", post(undo_change))
        .route("/spaces/{id}/nodes/{node_id}/versions", get(list_versions))
        .route(
            "/spaces/{id}/nodes/{node_id}/versions/{version}/restore",
            post(restore_version),
        )
        .route(
            "/spaces/{id}/nodes/{node_id}/share",
            get(get_share).post(create_share).delete(delete_share),
        )
        .route("/public/{token}", get(public_doc))
}
