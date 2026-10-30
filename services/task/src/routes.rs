//! HTTP 路由。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use club_auth_sdk::AuthUser;
use club_common::{new_id, AppError, FieldError};

use crate::entity::{board, column, comment, label, project, subtask, task, task_attachment};
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

/// 写入 outbox 事件（未配置总线时跳过）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn enqueue_event(
    state: &SharedState,
    event_type: &str,
    actor: Uuid,
    targets: Vec<Uuid>,
    task_id: Uuid,
    title: &str,
    body: &str,
    priority: &str,
) {
    if targets.is_empty() {
        return;
    }
    let payload = json!({
        "id": Uuid::now_v7(),
        "type": event_type,
        "actorId": actor,
        "targetUsers": targets.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "resource": { "type": "task", "id": task_id, "url": format!("/tasks/{task_id}") },
        "title": title,
        "body": body,
        "priority": priority
    });
    if let Err(err) = club_bus::outbox::enqueue(&state.db, event_type, &payload, Utc::now()).await {
        tracing::warn!(error = %err, "任务事件写入 outbox 失败");
    }
}

/// 看板列 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDto {
    /// ID。
    pub id: String,
    /// 所属看板。
    pub board_id: String,
    /// 名称。
    pub name: String,
    /// 排序。
    pub position: i64,
    /// 是否完成列。
    pub is_done: bool,
}

impl From<&column::Model> for ColumnDto {
    fn from(model: &column::Model) -> Self {
        Self {
            id: model.id.to_string(),
            board_id: model.board_id.to_string(),
            name: model.name.clone(),
            position: model.position,
            is_done: model.is_done,
        }
    }
}

/// 任务 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    /// ID。
    pub id: String,
    /// 项目。
    pub project_id: String,
    /// 列。
    pub column_id: String,
    /// 标题。
    pub title: String,
    /// 描述。
    pub description_md: String,
    /// 负责人（多负责人中的首个，兼容字段）。
    pub assignee_id: Option<String>,
    /// 负责人列表。
    pub assignee_ids: Vec<String>,
    /// 开始时间。
    pub start_at: Option<DateTime<FixedOffset>>,
    /// 状态：active/done/terminated。
    pub status: String,
    /// 优先级。
    pub priority: String,
    /// 截止时间。
    pub due_at: Option<DateTime<FixedOffset>>,
    /// 排序。
    pub position: i64,
    /// 完成时间。
    pub completed_at: Option<DateTime<FixedOffset>>,
    /// 更新时间。
    pub updated_at: DateTime<FixedOffset>,
    /// 标签。
    pub labels: Vec<LabelDto>,
    /// 封面附件。
    pub cover_attachment_id: Option<String>,
    /// 封面签名地址。
    pub cover_url: Option<String>,
}

/// 标签 DTO。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LabelDto {
    /// ID。
    pub id: String,
    /// 名称。
    pub name: String,
    /// 颜色。
    pub color: String,
}

impl From<&label::Model> for LabelDto {
    fn from(model: &label::Model) -> Self {
        Self {
            id: model.id.to_string(),
            name: model.name.clone(),
            color: model.color.clone(),
        }
    }
}

impl From<&task::Model> for TaskDto {
    fn from(model: &task::Model) -> Self {
        Self {
            id: model.id.to_string(),
            project_id: model.project_id.to_string(),
            column_id: model.column_id.to_string(),
            title: model.title.clone(),
            description_md: model.description_md.clone(),
            assignee_id: model.assignee_id.map(|id| id.to_string()),
            assignee_ids: model
                .assignee_id
                .map(|id| id.to_string())
                .into_iter()
                .collect(),
            start_at: model.start_at,
            status: model.status.clone(),
            priority: model.priority.clone(),
            due_at: model.due_at,
            position: model.position,
            completed_at: model.completed_at,
            updated_at: model.updated_at,
            labels: Vec::new(),
            cover_attachment_id: model.cover_attachment_id.map(|id| id.to_string()),
            cover_url: None,
        }
    }
}

/// 创建项目请求。
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    /// 名称。
    pub name: String,
    /// 描述。
    pub description: Option<String>,
}

/// `POST /projects`。
pub async fn create_project(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(input): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let user_id = user_id_of(&auth)?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "项目名称需为 1 ~ 64 字符",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let model = repo::create_project(
        &state.db,
        user_id,
        name,
        input.description.as_deref().unwrap_or(""),
        state.now(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": model.id, "name": model.name })),
    ))
}

/// `GET /projects`。
pub async fn list_projects(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let projects = repo::list_projects(&state.db, user_id).await?;
    Ok(Json(json!(projects
        .iter()
        .map(|project| json!({ "id": project.id, "name": project.name, "description": project.description }))
        .collect::<Vec<_>>())))
}

/// 添加成员请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMembersRequest {
    /// 用户列表。
    pub user_ids: Vec<Uuid>,
}

/// `POST /projects/{id}/members`（管理员）。
pub async fn add_members(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<AddMembersRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_admin(&state.db, project_id, user_id).await?;
    let mut added = 0;
    for member_id in &input.user_ids {
        repo::add_member(
            &state.db,
            project_id,
            *member_id,
            crate::entity::project_member::ROLE_MEMBER,
            state.now(),
        )
        .await?;
        added += 1;
    }
    Ok(Json(json!({ "added": added })))
}

/// `DELETE /columns/{id}`：删除空列（管理员）。
pub async fn delete_column(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(column_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let column = repo::find_column(&state.db, column_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_COLUMN_NOT_FOUND", "看板列不存在"))?;
    repo::ensure_admin(&state.db, column.project_id, user_id).await?;
    if repo::column_task_count(&state.db, column_id).await? > 0 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "列里还有任务，无法删除",
            vec![FieldError::new("columnId", "请先移走任务")],
        ));
    }
    repo::delete_column(&state.db, column_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 校验颜色格式（#rrggbb）。
fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

/// `GET /boards/{id}/labels`：看板标签。
pub async fn list_labels(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
) -> Result<Json<Vec<LabelDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let labels = repo::list_labels(&state.db, board_id).await?;
    Ok(Json(labels.iter().map(LabelDto::from).collect()))
}

/// 标签请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelRequest {
    /// 名称。
    pub name: String,
    /// 颜色（#rrggbb）。
    pub color: String,
}

/// `POST /boards/{id}/labels`：新建标签（管理员）。
pub async fn create_label(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(input): Json<LabelRequest>,
) -> Result<(StatusCode, Json<LabelDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_admin(&state.db, board.project_id, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 32 || !valid_color(&input.color) {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "标签名称或颜色不合法",
            vec![FieldError::new("name/color", "名称 1~32 字，颜色 #rrggbb")],
        ));
    }
    let model = repo::create_label(&state.db, board_id, name, &input.color, state.now()).await?;
    Ok((StatusCode::CREATED, Json(LabelDto::from(&model))))
}

/// `PATCH /labels/{id}`：更新标签。
pub async fn update_label(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(label_id): Path<Uuid>,
    Json(input): Json<LabelRequest>,
) -> Result<Json<LabelDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let label = repo::find_label(&state.db, label_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_LABEL_NOT_FOUND", "标签不存在"))?;
    let board = load_board(&state, label.board_id).await?;
    repo::ensure_admin(&state.db, board.project_id, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 32 || !valid_color(&input.color) {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "标签名称或颜色不合法",
            vec![FieldError::new("name/color", "名称 1~32 字，颜色 #rrggbb")],
        ));
    }
    let updated = repo::update_label(&state.db, &label, name, &input.color).await?;
    Ok(Json(LabelDto::from(&updated)))
}

/// `DELETE /labels/{id}`：删除标签。
pub async fn delete_label(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(label_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let label = repo::find_label(&state.db, label_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_LABEL_NOT_FOUND", "标签不存在"))?;
    let board = load_board(&state, label.board_id).await?;
    repo::ensure_admin(&state.db, board.project_id, user_id).await?;
    repo::delete_label(&state.db, label_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /tasks/{id}/labels/{label_id}`：给卡片打标签。
pub async fn attach_label(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let label = repo::find_label(&state.db, label_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_LABEL_NOT_FOUND", "标签不存在"))?;
    let column = repo::find_column(&state.db, model.column_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_COLUMN_NOT_FOUND", "看板列不存在"))?;
    if label.board_id != column.board_id {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "只能使用当前看板的标签",
            vec![FieldError::new("labelId", "标签不属于该看板")],
        ));
    }
    repo::attach_label(&state.db, task_id, label_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /tasks/{id}/labels/{label_id}`：取消卡片标签。
pub async fn detach_label(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    repo::detach_label(&state.db, task_id, label_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 增量同步参数。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncQuery {
    /// 上次同步时间（RFC3339）。
    pub since: Option<String>,
}

/// `GET /boards/{id}/sync?since=`：看板增量同步（轮询）。
pub async fn board_sync(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let since = match query.since.as_deref() {
        Some(raw) => chrono::DateTime::parse_from_rfc3339(raw)
            .map(|value| value.with_timezone(&chrono::Utc))
            .map_err(|_| {
                AppError::unprocessable(
                    "TASK_VALIDATION",
                    "since 不是合法时间",
                    vec![FieldError::new("since", "使用 RFC3339")],
                )
            })?,
        None => state.now() - chrono::Duration::seconds(30),
    };
    let tasks = repo::list_tasks_updated_since(&state.db, board_id, since).await?;
    let mut dtos = Vec::with_capacity(tasks.len());
    for model in &tasks {
        dtos.push(task_dto(&state, model).await?);
    }
    Ok(Json(json!({
        "serverTime": state.now().to_rfc3339(),
        "tasks": dtos,
    })))
}

/// 有效看板角色（继承或覆盖）。
async fn effective_board_role(
    state: &SharedState,
    board: &board::Model,
    user_id: Uuid,
) -> Result<(crate::entity::project_member::Model, String), AppError> {
    let member = repo::ensure_member(&state.db, board.project_id, user_id).await?;
    if member.role == "admin" {
        return Ok((member, "admin".to_string()));
    }
    let role = match repo::find_board_override(&state.db, board.id, user_id).await? {
        Some(row) => row.role,
        None => "editor".to_string(),
    };
    Ok((member, role))
}

/// 校验可写（editor/admin）。
async fn ensure_board_editor(
    state: &SharedState,
    board: &board::Model,
    user_id: Uuid,
) -> Result<(), AppError> {
    let (_, role) = effective_board_role(state, board, user_id).await?;
    if role == "viewer" {
        return Err(AppError::forbidden("TASK_FORBIDDEN", "看板只读，无法修改"));
    }
    Ok(())
}

/// 校验看板管理员。
async fn ensure_board_admin(
    state: &SharedState,
    board: &board::Model,
    user_id: Uuid,
) -> Result<(), AppError> {
    let (_, role) = effective_board_role(state, board, user_id).await?;
    if role != "admin" {
        return Err(AppError::forbidden("TASK_FORBIDDEN", "仅看板管理员可操作"));
    }
    Ok(())
}

/// `GET /boards/{id}/members`：看板成员与有效角色。
pub async fn list_board_members(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let members = repo::list_project_members(&state.db, board.project_id).await?;
    let overrides = repo::list_board_overrides(&state.db, board_id).await?;
    let items: Vec<Value> = members
        .iter()
        .map(|member| {
            let overridden = overrides.iter().find(|row| row.user_id == member.user_id);
            let role = if member.role == "admin" {
                "admin"
            } else {
                overridden.map(|row| row.role.as_str()).unwrap_or("editor")
            };
            json!({
                "userId": member.user_id,
                "role": role,
                "inherited": overridden.is_none(),
            })
        })
        .collect();
    Ok(Json(json!(items)))
}

/// 看板成员请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardMemberRequest {
    /// 用户。
    pub user_id: Uuid,
    /// editor / viewer / admin。
    pub role: String,
}

/// `POST /boards/{id}/members`：设置看板角色覆盖。
pub async fn set_board_member(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(input): Json<BoardMemberRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    ensure_board_admin(&state, &board, user_id).await?;
    if !["admin", "editor", "viewer"].contains(&input.role.as_str()) {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "看板角色不合法",
            vec![FieldError::new("role", "仅支持 admin/editor/viewer")],
        ));
    }
    repo::ensure_member(&state.db, board.project_id, input.user_id)
        .await
        .map_err(|_| AppError::not_found("TASK_MEMBER_NOT_FOUND", "目标不是项目成员"))?;
    let row =
        repo::upsert_board_member(&state.db, board_id, input.user_id, &input.role, state.now())
            .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "userId": row.user_id, "role": row.role })),
    ))
}

/// `DELETE /boards/{id}/members/{user_id}`：删除覆盖（回到继承）。
pub async fn delete_board_member(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((board_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    ensure_board_admin(&state, &board, user_id).await?;
    repo::delete_board_member(&state.db, board_id, target_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 看板 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardDto {
    /// ID。
    pub id: String,
    /// 名称。
    pub name: String,
    /// 排序。
    pub position: i64,
}

impl From<&board::Model> for BoardDto {
    fn from(model: &board::Model) -> Self {
        Self {
            id: model.id.to_string(),
            name: model.name.clone(),
            position: model.position,
        }
    }
}

/// `GET /projects/{id}/boards`：项目看板列表。
pub async fn list_boards(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<BoardDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, project_id, user_id).await?;
    let boards = repo::list_boards(&state.db, project_id).await?;
    Ok(Json(boards.iter().map(BoardDto::from).collect()))
}

/// 建看板请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardRequest {
    /// 名称。
    pub name: String,
}

/// `POST /projects/{id}/boards`：新建看板（管理员）。
pub async fn create_board(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<BoardRequest>,
) -> Result<(StatusCode, Json<BoardDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_admin(&state.db, project_id, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "看板名称需为 1 ~ 64 字符",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let model = repo::create_board(&state.db, project_id, name, state.now()).await?;
    Ok((StatusCode::CREATED, Json(BoardDto::from(&model))))
}

/// 校验看板可访问，返回看板。
async fn load_board(state: &SharedState, board_id: Uuid) -> Result<board::Model, AppError> {
    repo::find_board(&state.db, board_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_BOARD_NOT_FOUND", "看板不存在"))
}

/// `PATCH /boards/{id}`：重命名看板。
pub async fn rename_board(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(input): Json<BoardRequest>,
) -> Result<Json<BoardDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    ensure_board_admin(&state, &board, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "看板名称需为 1 ~ 64 字符",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let updated = repo::rename_board(&state.db, &board, name).await?;
    Ok(Json(BoardDto::from(&updated)))
}

/// `DELETE /boards/{id}`：删除空看板。
pub async fn delete_board(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    ensure_board_admin(&state, &board, user_id).await?;
    if repo::board_column_count(&state.db, board_id).await? > 0 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "看板里还有列，无法删除",
            vec![FieldError::new("boardId", "请先清空看板")],
        ));
    }
    repo::delete_board(&state.db, board_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /boards/{id}/columns`：看板列。
pub async fn list_board_columns(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
) -> Result<Json<Vec<ColumnDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let columns = repo::list_columns_by_board(&state.db, board_id).await?;
    Ok(Json(columns.iter().map(ColumnDto::from).collect()))
}

/// `POST /boards/{id}/columns`：在看板下建列。
pub async fn create_board_column(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(input): Json<CreateColumnRequest>,
) -> Result<(StatusCode, Json<ColumnDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    ensure_board_editor(&state, &board, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "列名称不合法",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let model =
        repo::create_column(&state.db, board.project_id, board.id, name, state.now()).await?;
    Ok((StatusCode::CREATED, Json(ColumnDto::from(&model))))
}

/// 看板任务筛选参数。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardTaskQuery {
    /// 负责人。
    pub assignee_id: Option<Uuid>,
    /// 标签。
    pub label_id: Option<Uuid>,
    /// 关键词。
    pub q: Option<String>,
    /// 截止：overdue / today / week。
    pub due: Option<String>,
}

/// `GET /boards/{id}/tasks`：看板任务（支持筛选）。
pub async fn list_board_tasks(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
    Query(query): Query<BoardTaskQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let filter = repo::TaskFilter {
        assignee_id: query.assignee_id,
        label_id: query.label_id,
        keyword: query.q,
        due: query.due,
    };
    let tasks = repo::list_tasks_by_board(&state.db, board_id, &filter, state.now()).await?;
    let mut dtos = Vec::with_capacity(tasks.len());
    for model in &tasks {
        dtos.push(task_dto(&state, model).await?);
    }
    Ok(Json(json!(dtos)))
}

/// `GET /boards/{id}/archived-tasks`：归档卡片。
pub async fn list_archived_tasks(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(board_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let board = load_board(&state, board_id).await?;
    repo::ensure_member(&state.db, board.project_id, user_id).await?;
    let tasks = repo::list_archived_tasks(&state.db, board_id).await?;
    let mut dtos = Vec::with_capacity(tasks.len());
    for model in &tasks {
        dtos.push(task_dto(&state, model).await?);
    }
    Ok(Json(json!(dtos)))
}

/// `POST /tasks/{id}/archive`：归档卡片。
pub async fn archive_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    repo::archive_task(&state.db, &model, state.now(), true).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /tasks/{id}/restore`：恢复归档卡片。
pub async fn restore_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    repo::archive_task(&state.db, &model, state.now(), false).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /projects/{id}/columns`。
pub async fn list_columns(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ColumnDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, project_id, user_id).await?;
    let columns = repo::list_columns(&state.db, project_id).await?;
    Ok(Json(columns.iter().map(ColumnDto::from).collect()))
}

/// 创建列请求。
#[derive(Debug, Deserialize)]
pub struct CreateColumnRequest {
    /// 名称。
    pub name: String,
}

/// `POST /projects/{id}/columns`。
pub async fn create_column(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateColumnRequest>,
) -> Result<(StatusCode, Json<ColumnDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_admin(&state.db, project_id, user_id).await?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "列名称不合法",
            vec![FieldError::new("name", "非法")],
        ));
    }
    let board = repo::default_board(&state.db, project_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_BOARD_NOT_FOUND", "默认看板不存在"))?;
    let model = repo::create_column(&state.db, project_id, board.id, name, state.now()).await?;
    Ok((StatusCode::CREATED, Json(ColumnDto::from(&model))))
}

/// 任务查询。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskQuery {
    /// 按列过滤。
    pub column_id: Option<Uuid>,
    /// 按负责人过滤。
    pub assignee_id: Option<Uuid>,
}

/// `GET /projects/{id}/tasks`。
pub async fn list_tasks(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<Vec<TaskDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, project_id, user_id).await?;
    let tasks = repo::list_tasks(&state.db, project_id, query.column_id, query.assignee_id).await?;
    let mut items = Vec::with_capacity(tasks.len());
    for model in &tasks {
        items.push(task_dto(&state, model).await?);
    }
    Ok(Json(items))
}

/// 加载负责人列表并构造 DTO。
async fn task_dto(state: &SharedState, model: &task::Model) -> Result<TaskDto, AppError> {
    let mut dto = TaskDto::from(model);
    let assignees = repo::list_task_assignees(&state.db, model.id).await?;
    if !assignees.is_empty() {
        dto.assignee_ids = assignees.iter().map(|id| id.to_string()).collect();
        dto.assignee_id = assignees.first().map(|id| id.to_string());
    }
    let mut labels = repo::labels_for_tasks(&state.db, &[model.id]).await?;
    if let Some(rows) = labels.remove(&model.id) {
        dto.labels = rows.iter().map(LabelDto::from).collect();
    }
    if let Some(attachment_id) = model.cover_attachment_id {
        if let Ok(token) = sign_attachment_token(&state.config.internal_secret, attachment_id) {
            dto.cover_url = Some(format!(
                "/api/v1/task/attachments/{attachment_id}/file?token={token}"
            ));
        }
    }
    Ok(dto)
}

/// 创建任务请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    /// 列（`/columns/{id}/tasks` 通道可省略）。
    pub column_id: Option<Uuid>,
    /// 标题。
    pub title: String,
    /// 描述。
    pub description_md: Option<String>,
    /// 负责人（单人选，兼容）。
    pub assignee_id: Option<Uuid>,
    /// 负责人列表（多人）。
    pub assignee_ids: Option<Vec<Uuid>>,
    /// 优先级。
    pub priority: Option<String>,
    /// 截止时间（RFC3339）。
    pub due_at: Option<DateTime<Utc>>,
    /// 开始时间（RFC3339）。
    pub start_at: Option<DateTime<Utc>>,
}

/// `POST /projects/{id}/tasks`。
/// `POST /projects/{id}/tasks`。
pub async fn create_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    repo::ensure_member(&state.db, project_id, user_id).await?;
    let column_id = input.column_id.ok_or_else(|| {
        AppError::unprocessable(
            "TASK_VALIDATION",
            "缺少 columnId",
            vec![FieldError::new("columnId", "不能为空")],
        )
    })?;
    let column = repo::find_column(&state.db, column_id)
        .await?
        .filter(|column| column.project_id == project_id)
        .ok_or_else(|| AppError::not_found("TASK_COLUMN_NOT_FOUND", "看板列不存在"))?;
    create_task_core(&state, user_id, project_id, column, input).await
}

/// `POST /columns/{id}/tasks`：按列创建任务（项目由列推导）。
pub async fn create_task_in_column(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(input): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    let column = repo::find_column(&state.db, column_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_COLUMN_NOT_FOUND", "看板列不存在"))?;
    let board = repo::find_board(&state.db, column.board_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_BOARD_NOT_FOUND", "看板不存在"))?;
    ensure_board_editor(&state, &board, user_id).await?;
    create_task_core(&state, user_id, column.project_id, column, input).await
}

async fn create_task_core(
    state: &SharedState,
    user_id: Uuid,
    project_id: Uuid,
    column: column::Model,
    input: CreateTaskRequest,
) -> Result<(StatusCode, Json<TaskDto>), AppError> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 255 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "标题需为 1 ~ 255 字符",
            vec![FieldError::new("title", "非法")],
        ));
    }
    let priority = input.priority.unwrap_or_else(|| "normal".to_string());
    if !["low", "normal", "high", "urgent"].contains(&priority.as_str()) {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "优先级不合法",
            vec![FieldError::new("priority", "仅支持 low/normal/high/urgent")],
        ));
    }
    let mut assignees = input.assignee_ids.unwrap_or_default();
    if assignees.is_empty() {
        assignees.extend(input.assignee_id);
    }
    assignees.dedup();
    let model = repo::create_task(
        &state.db,
        project_id,
        column.id,
        title,
        input.description_md.as_deref().unwrap_or(""),
        assignees.first().copied(),
        &priority,
        input.due_at,
        input.start_at,
        user_id,
        state.now(),
    )
    .await?;
    if !assignees.is_empty() {
        repo::set_task_assignees(&state.db, model.id, &assignees, state.now()).await?;
    }
    for assignee in &assignees {
        if *assignee != user_id {
            enqueue_event(
                state,
                "task.assigned",
                user_id,
                vec![*assignee],
                model.id,
                "有新任务指派给你",
                &model.title,
                "high",
            )
            .await;
        }
    }
    Ok((StatusCode::CREATED, Json(task_dto(state, &model).await?)))
}

/// 加载任务并校验项目成员。
async fn load_task_for_member(
    state: &SharedState,
    user_id: Uuid,
    task_id: Uuid,
) -> Result<task::Model, AppError> {
    let model = repo::find_task(&state.db, task_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_NOT_FOUND", "任务不存在"))?;
    repo::ensure_member(&state.db, model.project_id, user_id).await?;
    Ok(model)
}

/// `GET /tasks/{id}`。
pub async fn get_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    Ok(Json(task_dto(&state, &model).await?))
}

/// 三态 JSON：字段缺省 = None，显式 null = Some(None)，有值 = Some(Some(v))。
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 更新任务请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    /// 标题。
    pub title: Option<String>,
    /// 描述。
    pub description_md: Option<String>,
    /// 负责人（null 表示清空）。
    #[serde(default, deserialize_with = "double_option")]
    pub assignee_id: Option<Option<Uuid>>,
    /// 负责人列表（覆盖式；空数组表示清空）。
    pub assignee_ids: Option<Vec<Uuid>>,
    /// 优先级。
    pub priority: Option<String>,
    /// 截止时间（null 表示清空）。
    #[serde(default, deserialize_with = "double_option")]
    pub due_at: Option<Option<DateTime<Utc>>>,
    /// 开始时间（null 表示清空）。
    #[serde(default, deserialize_with = "double_option")]
    pub start_at: Option<Option<DateTime<Utc>>>,
}

/// `PATCH /tasks/{id}`。
pub async fn update_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<UpdateTaskRequest>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    if let Some(priority) = input.priority.as_deref() {
        if !["low", "normal", "high", "urgent"].contains(&priority) {
            return Err(AppError::unprocessable(
                "TASK_VALIDATION",
                "优先级不合法",
                vec![FieldError::new("priority", "非法")],
            ));
        }
    }
    let before_snapshot = task_snapshot(&state, &model).await?;
    let old_assignees = repo::list_task_assignees(&state.db, model.id).await?;
    let legacy_old: Vec<Uuid> = if old_assignees.is_empty() {
        model.assignee_id.into_iter().collect()
    } else {
        old_assignees
    };
    let mut replaced: Option<Vec<Uuid>> = None;
    let assignee_field = if let Some(mut list) = input.assignee_ids {
        list.dedup();
        let first = list.first().copied();
        replaced = Some(list);
        Some(first)
    } else {
        input.assignee_id
    };
    let updated = repo::update_task(
        &state.db,
        &model,
        input.title,
        input.description_md,
        assignee_field,
        input.priority,
        input.due_at,
        input.start_at,
        state.now(),
    )
    .await?;
    if let Some(list) = replaced {
        repo::set_task_assignees(&state.db, updated.id, &list, state.now()).await?;
        for assignee in &list {
            if *assignee != user_id && !legacy_old.contains(assignee) {
                enqueue_event(
                    &state,
                    "task.assigned",
                    user_id,
                    vec![*assignee],
                    updated.id,
                    "有新任务指派给你",
                    &updated.title,
                    "high",
                )
                .await;
            }
        }
    } else if let Some(Some(assignee)) = input.assignee_id {
        repo::set_task_assignees(&state.db, updated.id, &[assignee], state.now()).await?;
        if assignee != user_id && !legacy_old.contains(&assignee) {
            enqueue_event(
                &state,
                "task.assigned",
                user_id,
                vec![assignee],
                updated.id,
                "有新任务指派给你",
                &updated.title,
                "high",
            )
            .await;
        }
    } else if matches!(input.assignee_id, Some(None)) {
        repo::set_task_assignees(&state.db, updated.id, &[], state.now()).await?;
    }
    let after_snapshot = task_snapshot(&state, &updated).await?;
    let _ = club_bus::audit::record(
        &state.db,
        "task",
        &updated.id.to_string(),
        "update",
        Some(before_snapshot),
        Some(after_snapshot),
        Some(user_id),
        state.now(),
    )
    .await;
    Ok(Json(task_dto(&state, &updated).await?))
}

/// 更新任务状态请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskStatusRequest {
    /// active / done / terminated。
    pub status: String,
}

/// `POST /tasks/{id}/status`。
pub async fn update_task_status(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<UpdateTaskStatusRequest>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    if !["active", "done", "terminated"].contains(&input.status.as_str()) {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "状态不合法",
            vec![FieldError::new("status", "仅支持 active/done/terminated")],
        ));
    }
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let before_snapshot = task_snapshot(&state, &model).await?;
    let updated = repo::update_task_status(&state.db, &model, &input.status, state.now()).await?;
    let after_snapshot = task_snapshot(&state, &updated).await?;
    let _ = club_bus::audit::record(
        &state.db,
        "task",
        &updated.id.to_string(),
        "status",
        Some(before_snapshot),
        Some(after_snapshot),
        Some(user_id),
        state.now(),
    )
    .await;
    if matches!(input.status.as_str(), "done" | "terminated") {
        let mut targets = repo::list_task_assignees(&state.db, updated.id).await?;
        if targets.is_empty() {
            targets.extend(updated.assignee_id);
        }
        targets.retain(|id| *id != user_id);
        if !targets.is_empty() {
            let event_type = if input.status == "done" {
                "task.completed"
            } else {
                "task.terminated"
            };
            enqueue_event(
                &state,
                event_type,
                user_id,
                targets,
                updated.id,
                "任务状态更新",
                &updated.title,
                "normal",
            )
            .await;
        }
    }
    Ok(Json(task_dto(&state, &updated).await?))
}

/// `POST /tasks/{id}/claim`：认领任务（把自己加入负责人）。
pub async fn claim_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let mut assignees = repo::list_task_assignees(&state.db, model.id).await?;
    if assignees.is_empty() {
        assignees.extend(model.assignee_id);
    }
    if !assignees.contains(&user_id) {
        assignees.push(user_id);
    }
    repo::set_task_assignees(&state.db, model.id, &assignees, state.now()).await?;
    let first = assignees.first().copied();
    let updated = repo::update_task(
        &state.db,
        &model,
        None,
        None,
        Some(first),
        None,
        None,
        None,
        state.now(),
    )
    .await?;
    Ok(Json(task_dto(&state, &updated).await?))
}

/// `DELETE /tasks/{id}`。
pub async fn delete_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let before_snapshot = task_snapshot(&state, &model).await?;
    repo::delete_task(&state.db, model.id).await?;
    let _ = club_bus::audit::record(
        &state.db,
        "task",
        &model.id.to_string(),
        "delete",
        Some(before_snapshot),
        None,
        Some(user_id),
        state.now(),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// 附件 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentDto {
    /// ID。
    pub id: String,
    /// 文件名。
    pub name: String,
    /// 字节数。
    pub size: i64,
    /// MIME。
    pub mime: String,
    /// 上传者。
    pub uploader_id: String,
    /// 上传时间。
    pub created_at: DateTime<FixedOffset>,
}

impl From<&task_attachment::Model> for AttachmentDto {
    fn from(model: &task_attachment::Model) -> Self {
        Self {
            id: model.id.to_string(),
            name: model.name.clone(),
            size: model.size,
            mime: model.mime.clone(),
            uploader_id: model.uploader_id.to_string(),
            created_at: model.created_at,
        }
    }
}

/// 附件签名有效期：365 天（URL 会写进卡片封面）。
const ATTACHMENT_TOKEN_TTL_SECONDS: i64 = 365 * 24 * 3600;

/// 附件签名载荷。
#[derive(Debug, Serialize, Deserialize)]
struct AttachmentTokenClaims {
    /// 附件 ID。
    sub: String,
    /// 过期时间戳。
    exp: i64,
}

/// 生成附件下载签名（HS256）。
fn sign_attachment_token(secret: &str, attachment_id: Uuid) -> Result<String, AppError> {
    let claims = AttachmentTokenClaims {
        sub: attachment_id.to_string(),
        exp: chrono::Utc::now().timestamp() + ATTACHMENT_TOKEN_TTL_SECONDS,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

/// 附件签名查询参数。
#[derive(Debug, Deserialize)]
pub struct AttachmentTokenQuery {
    /// 签名。
    pub token: String,
}

/// `GET /attachments/{id}/file?token=`：签名下载（`<img>` 可用）。
pub async fn public_attachment(
    State(state): State<SharedState>,
    Path(attachment_id): Path<Uuid>,
    Query(query): Query<AttachmentTokenQuery>,
) -> Result<axum::response::Response, AppError> {
    use axum::response::IntoResponse;
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    let decoded = jsonwebtoken::decode::<AttachmentTokenClaims>(
        &query.token,
        &jsonwebtoken::DecodingKey::from_secret(state.config.internal_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "下载链接无效或已过期"))?;
    if decoded.claims.sub != attachment_id.to_string() {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_TOKEN",
            "下载链接与附件不匹配",
        ));
    }
    let model = task_attachment::Entity::find_by_id(attachment_id)
        .one(&state.db)
        .await
        .map_err(repo::map_db_err)?
        .ok_or_else(|| AppError::not_found("TASK_ATTACHMENT_NOT_FOUND", "附件不存在"))?;
    let path = std::path::Path::new(&state.config.storage_path).join(&model.storage_key);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::not_found("TASK_ATTACHMENT_NOT_FOUND", "附件文件缺失"))?;
    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, model.mime.clone()),
            (
                axum::http::header::CACHE_CONTROL,
                "private, max-age=86400".to_string(),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// 设置封面请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverRequest {
    /// 附件 ID。
    pub attachment_id: Uuid,
}

/// `POST /tasks/{id}/cover`：设置卡片封面。
pub async fn set_cover(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<CoverRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let attachment = task_attachment::Entity::find_by_id(input.attachment_id)
        .filter(task_attachment::Column::TaskId.eq(task_id))
        .one(&state.db)
        .await
        .map_err(repo::map_db_err)?
        .ok_or_else(|| {
            AppError::unprocessable(
                "TASK_VALIDATION",
                "附件不属于该任务",
                vec![FieldError::new("attachmentId", "附件不属于该任务")],
            )
        })?;
    let mut active: task::ActiveModel = model.into();
    active.cover_attachment_id = Set(Some(attachment.id));
    active.update(&state.db).await.map_err(repo::map_db_err)?;
    Ok(Json(json!({ "coverAttachmentId": attachment.id })))
}

/// `DELETE /tasks/{id}/cover`：取消封面。
pub async fn clear_cover(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let mut active: task::ActiveModel = model.into();
    active.cover_attachment_id = Set(None);
    active.update(&state.db).await.map_err(repo::map_db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 附件上传查询参数。
#[derive(Debug, Deserialize)]
pub struct AttachmentUploadQuery {
    /// 文件名。
    pub name: String,
    /// MIME（可选）。
    pub mime: Option<String>,
}

/// 简易百分号编码（用于 Content-Disposition 的 UTF-8 文件名）。
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// `GET /tasks/{id}/attachments`。
pub async fn list_attachments(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<AttachmentDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let rows = task_attachment::Entity::find()
        .filter(task_attachment::Column::TaskId.eq(task_id))
        .order_by_asc(task_attachment::Column::CreatedAt)
        .all(&state.db)
        .await
        .map_err(repo::map_db_err)?;
    Ok(Json(rows.iter().map(AttachmentDto::from).collect()))
}

/// `POST /tasks/{id}/attachments?name=&mime=`（原始字节体）。
pub async fn upload_attachment(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Query(query): Query<AttachmentUploadQuery>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<AttachmentDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let name = query.name.trim();
    if name.is_empty() || name.chars().count() > 255 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "文件名需为 1 ~ 255 字符",
            vec![FieldError::new("name", "非法")],
        ));
    }
    if body.len() > 50 * 1024 * 1024 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "附件不能超过 50MB",
            vec![FieldError::new("file", "过大")],
        ));
    }
    let attachment_id = new_id();
    let storage_key = format!("{task_id}/{attachment_id}");
    let dir = std::path::Path::new(&state.config.storage_path).join(task_id.to_string());
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(AppError::internal)?;
    tokio::fs::write(dir.join(attachment_id.to_string()), &body)
        .await
        .map_err(AppError::internal)?;
    let mime = query
        .mime
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let model = task_attachment::ActiveModel {
        id: Set(attachment_id),
        task_id: Set(task_id),
        name: Set(name.to_string()),
        size: Set(body.len() as i64),
        mime: Set(mime),
        storage_key: Set(storage_key),
        uploader_id: Set(user_id),
        created_at: Set(state.now().fixed_offset()),
    }
    .insert(&state.db)
    .await
    .map_err(repo::map_db_err)?;
    Ok((StatusCode::CREATED, Json(AttachmentDto::from(&model))))
}

/// `GET /tasks/{id}/attachments/{attachment_id}/download`。
pub async fn download_attachment(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, AppError> {
    use axum::response::IntoResponse;
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let model = task_attachment::Entity::find_by_id(attachment_id)
        .filter(task_attachment::Column::TaskId.eq(task_id))
        .one(&state.db)
        .await
        .map_err(repo::map_db_err)?
        .ok_or_else(|| AppError::not_found("TASK_ATTACHMENT_NOT_FOUND", "附件不存在"))?;
    let path = std::path::Path::new(&state.config.storage_path).join(&model.storage_key);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::not_found("TASK_ATTACHMENT_NOT_FOUND", "附件文件缺失"))?;
    let disposition = format!(
        "attachment; filename*=UTF-8''{}",
        percent_encode(&model.name)
    );
    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, model.mime.clone()),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}

/// `DELETE /tasks/{id}/attachments/{attachment_id}`。
pub async fn delete_attachment(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let model = task_attachment::Entity::find_by_id(attachment_id)
        .filter(task_attachment::Column::TaskId.eq(task_id))
        .one(&state.db)
        .await
        .map_err(repo::map_db_err)?
        .ok_or_else(|| AppError::not_found("TASK_ATTACHMENT_NOT_FOUND", "附件不存在"))?;
    task_attachment::Entity::delete_by_id(attachment_id)
        .exec(&state.db)
        .await
        .map_err(repo::map_db_err)?;
    let path = std::path::Path::new(&state.config.storage_path).join(&model.storage_key);
    let _ = tokio::fs::remove_file(path).await;
    Ok(StatusCode::NO_CONTENT)
}

/// 更新项目请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    /// 名称。
    pub name: Option<String>,
    /// 描述。
    pub description: Option<String>,
}

/// 校验项目管理员（项目成员中的 admin 或创建者）。
async fn ensure_project_admin(
    state: &SharedState,
    project: &project::Model,
    user_id: Uuid,
) -> Result<(), AppError> {
    if project.owner_id == user_id {
        return Ok(());
    }
    let member = repo::ensure_member(&state.db, project.id, user_id).await?;
    if member.role == "admin" {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "TASK_FORBIDDEN",
            "仅项目管理员可执行该操作",
        ))
    }
}

/// `PATCH /projects/{id}`：编辑项目。
pub async fn update_project(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<UpdateProjectRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let project = repo::find_project(&state.db, project_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_PROJECT_NOT_FOUND", "项目不存在"))?;
    ensure_project_admin(&state, &project, user_id).await?;
    if let Some(name) = input.name.as_deref() {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(AppError::unprocessable(
                "TASK_VALIDATION",
                "项目名称需为 1 ~ 64 字符",
                vec![FieldError::new("name", "非法")],
            ));
        }
    }
    let updated = repo::update_project(
        &state.db,
        &project,
        input.name.map(|name| name.trim().to_string()),
        input.description,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "id": updated.id, "name": updated.name, "description": updated.description }),
    ))
}

/// `DELETE /projects/{id}`：删除项目及其全部任务。
pub async fn delete_project(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let project = repo::find_project(&state.db, project_id)
        .await?
        .ok_or_else(|| AppError::not_found("TASK_PROJECT_NOT_FOUND", "项目不存在"))?;
    ensure_project_admin(&state, &project, user_id).await?;
    repo::delete_project(&state.db, project_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 任务快照（审计/撤销用）。
async fn task_snapshot(
    state: &SharedState,
    model: &task::Model,
) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(task_dto(state, model).await?).map_err(AppError::internal)
}

/// 从快照解析可空时间（缺省 = None 不修改；null = 清空）。
fn snapshot_dt(snapshot: &serde_json::Value, key: &str) -> Option<Option<DateTime<Utc>>> {
    match snapshot.get(key) {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(serde_json::Value::String(text)) => Some(
            DateTime::parse_from_rfc3339(text)
                .ok()
                .map(|value| value.with_timezone(&Utc)),
        ),
        _ => None,
    }
}

/// 从快照解析负责人列表。
fn snapshot_assignees(snapshot: &serde_json::Value) -> Vec<Uuid> {
    snapshot
        .get("assigneeIds")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter_map(|value| value.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// `GET /tasks/{id}/changes`：任务操作记录。
pub async fn list_task_changes(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<club_bus::audit::ChangeEntry>>, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let items = club_bus::audit::list_for(&state.db, "task", &task_id.to_string(), 50)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(items))
}

/// `POST /changes/{id}/undo`：撤销一次任务变更（删除类按快照重建）。
pub async fn undo_change(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(change_id): Path<Uuid>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let entry = club_bus::audit::find(&state.db, change_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("TASK_CHANGE_NOT_FOUND", "变更记录不存在"))?;
    if entry.entity != "task" {
        return Err(AppError::unprocessable(
            "TASK_UNDO_UNSUPPORTED",
            "仅支持撤销任务变更",
            vec![],
        ));
    }
    let before = entry.before.clone().ok_or_else(|| {
        AppError::unprocessable("TASK_UNDO_UNSUPPORTED", "该记录不可撤销", vec![])
    })?;
    let now = state.now();
    let updated = if entry.action == "delete" {
        let project_id: Uuid = before
            .get("projectId")
            .and_then(serde_json::Value::as_str)
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| {
                AppError::unprocessable("TASK_UNDO_UNSUPPORTED", "快照缺少项目", vec![])
            })?;
        repo::ensure_member(&state.db, project_id, user_id).await?;
        let model = repo::restore_task(&state.db, &before, user_id, now).await?;
        repo::set_task_assignees(&state.db, model.id, &snapshot_assignees(&before), now).await?;
        model
    } else {
        let task_id: Uuid = entry.entity_id.parse().map_err(AppError::internal)?;
        let model = load_task_for_member(&state, user_id, task_id).await?;
        let assignees = snapshot_assignees(&before);
        let updated = repo::update_task(
            &state.db,
            &model,
            before
                .get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            before
                .get("descriptionMd")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            Some(assignees.first().copied()),
            before
                .get("priority")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            snapshot_dt(&before, "dueAt"),
            snapshot_dt(&before, "startAt"),
            now,
        )
        .await?;
        repo::set_task_assignees(&state.db, updated.id, &assignees, now).await?;
        let status = before
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active");
        repo::update_task_status(&state.db, &updated, status, now).await?
    };
    let snapshot = task_snapshot(&state, &updated).await?;
    let _ = club_bus::audit::record(
        &state.db,
        "task",
        &updated.id.to_string(),
        "undo",
        Some(snapshot.clone()),
        None,
        Some(user_id),
        now,
    )
    .await;
    Ok(Json(task_dto(&state, &updated).await?))
}

/// 移动任务请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTaskRequest {
    /// 目标列。
    pub column_id: Uuid,
}

/// `POST /tasks/{id}/move`。
pub async fn move_task(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<MoveTaskRequest>,
) -> Result<Json<TaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let target = repo::find_column(&state.db, input.column_id)
        .await?
        .filter(|column| column.project_id == model.project_id)
        .ok_or_else(|| AppError::not_found("TASK_COLUMN_NOT_FOUND", "看板列不存在"))?;
    let moved = repo::move_task(&state.db, &model, &target, state.now()).await?;
    Ok(Json(TaskDto::from(&moved)))
}

/// 子任务 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskDto {
    /// ID。
    pub id: String,
    /// 标题。
    pub title: String,
    /// 是否完成。
    pub done: bool,
}

impl From<&subtask::Model> for SubtaskDto {
    fn from(model: &subtask::Model) -> Self {
        Self {
            id: model.id.to_string(),
            title: model.title.clone(),
            done: model.done,
        }
    }
}

/// 创建子任务请求。
#[derive(Debug, Deserialize)]
pub struct CreateSubtaskRequest {
    /// 标题。
    pub title: String,
}

/// `GET /tasks/{id}/subtasks`。
pub async fn list_subtasks(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<SubtaskDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let items = repo::list_subtasks(&state.db, task_id).await?;
    Ok(Json(items.iter().map(SubtaskDto::from).collect()))
}

/// `POST /tasks/{id}/subtasks`。
pub async fn create_subtask(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<CreateSubtaskRequest>,
) -> Result<(StatusCode, Json<SubtaskDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "子任务标题不能为空",
            vec![FieldError::new("title", "不能为空")],
        ));
    }
    let model = repo::create_subtask(&state.db, task_id, title, state.now()).await?;
    Ok((StatusCode::CREATED, Json(SubtaskDto::from(&model))))
}

/// 切换子任务请求。
#[derive(Debug, Deserialize)]
pub struct ToggleSubtaskRequest {
    /// 是否完成。
    pub done: bool,
}

/// `PATCH /subtasks/{id}`。
pub async fn toggle_subtask(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(subtask_id): Path<Uuid>,
    Json(input): Json<ToggleSubtaskRequest>,
) -> Result<Json<SubtaskDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model = subtask::Entity::find_by_id(subtask_id)
        .one(&state.db)
        .await
        .map_err(repo::map_db_err)?
        .ok_or_else(|| AppError::not_found("TASK_SUBTASK_NOT_FOUND", "子任务不存在"))?;
    load_task_for_member(&state, user_id, model.task_id).await?;
    let updated = repo::set_subtask_done(&state.db, &model, input.done).await?;
    Ok(Json(SubtaskDto::from(&updated)))
}

/// 评论 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    /// ID。
    pub id: String,
    /// 作者。
    pub author_id: String,
    /// 内容。
    pub content_md: String,
    /// 时间。
    pub created_at: DateTime<FixedOffset>,
}

impl From<&comment::Model> for CommentDto {
    fn from(model: &comment::Model) -> Self {
        Self {
            id: model.id.to_string(),
            author_id: model.author_id.to_string(),
            content_md: model.content_md.clone(),
            created_at: model.created_at,
        }
    }
}

/// 创建评论请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    /// 内容。
    pub content_md: String,
    /// `@` 的用户。
    #[serde(default)]
    pub mentions: Vec<Uuid>,
}

/// `GET /tasks/{id}/comments`。
pub async fn list_comments(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<CommentDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    load_task_for_member(&state, user_id, task_id).await?;
    let items = repo::list_comments(&state.db, task_id).await?;
    Ok(Json(items.iter().map(CommentDto::from).collect()))
}

/// `POST /tasks/{id}/comments`。
pub async fn create_comment(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    let model = load_task_for_member(&state, user_id, task_id).await?;
    let content = input.content_md.trim();
    if content.is_empty() || content.chars().count() > 4096 {
        return Err(AppError::unprocessable(
            "TASK_VALIDATION",
            "评论内容需为 1 ~ 4096 字符",
            vec![FieldError::new("contentMd", "非法")],
        ));
    }
    let comment = repo::create_comment(&state.db, task_id, user_id, content, state.now()).await?;
    let mut targets: Vec<Uuid> = repo::list_task_assignees(&state.db, task_id).await?;
    targets.extend(model.assignee_id);
    targets.extend(input.mentions.iter().copied());
    targets.retain(|id| *id != user_id);
    targets.sort();
    targets.dedup();
    enqueue_event(
        &state,
        "task.comment.added",
        user_id,
        targets,
        task_id,
        "任务有新评论",
        &model.title,
        "normal",
    )
    .await;
    Ok((StatusCode::CREATED, Json(CommentDto::from(&comment))))
}

/// 我的任务查询。
#[derive(Debug, Deserialize)]
pub struct MyTaskQuery {
    /// overdue / today / week（空 = 全部未完成）。
    pub due: Option<String>,
}

/// `GET /my/tasks`。
pub async fn my_tasks(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<MyTaskQuery>,
) -> Result<Json<Vec<TaskDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let now = state.now();
    let (due_before, only_open) = match query.due.as_deref() {
        Some("overdue") => (Some(now), true),
        Some("today") => (Some(now + Duration::days(1)), true),
        Some("week") => (Some(now + Duration::days(7)), true),
        _ => (None, true),
    };
    let items = repo::my_tasks(&state.db, user_id, due_before, only_open).await?;
    let mut dtos = Vec::with_capacity(items.len());
    for model in &items {
        dtos.push(task_dto(&state, model).await?);
    }
    Ok(Json(dtos))
}

/// `/api/v1/task` 路由。
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{id}/members", post(add_members))
        .route(
            "/projects/{id}",
            axum::routing::patch(update_project).delete(delete_project),
        )
        .route(
            "/projects/{id}/columns",
            get(list_columns).post(create_column),
        )
        .route("/projects/{id}/boards", get(list_boards).post(create_board))
        .route(
            "/boards/{id}",
            axum::routing::patch(rename_board).delete(delete_board),
        )
        .route(
            "/boards/{id}/columns",
            get(list_board_columns).post(create_board_column),
        )
        .route("/boards/{id}/tasks", get(list_board_tasks))
        .route("/boards/{id}/sync", get(board_sync))
        .route("/boards/{id}/archived-tasks", get(list_archived_tasks))
        .route("/tasks/{id}/archive", post(archive_task))
        .route("/tasks/{id}/restore", post(restore_task))
        .route(
            "/boards/{id}/members",
            get(list_board_members).post(set_board_member),
        )
        .route(
            "/boards/{id}/members/{user_id}",
            axum::routing::delete(delete_board_member),
        )
        .route("/boards/{id}/labels", get(list_labels).post(create_label))
        .route(
            "/labels/{id}",
            axum::routing::patch(update_label).delete(delete_label),
        )
        .route(
            "/tasks/{id}/labels/{label_id}",
            post(attach_label).delete(detach_label),
        )
        .route("/columns/{id}", axum::routing::delete(delete_column))
        .route("/projects/{id}/tasks", get(list_tasks).post(create_task))
        .route("/columns/{id}/tasks", post(create_task_in_column))
        .route(
            "/tasks/{id}",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .route("/tasks/{id}/status", post(update_task_status))
        .route("/tasks/{id}/claim", post(claim_task))
        .route("/tasks/{id}/changes", get(list_task_changes))
        .route("/changes/{id}/undo", post(undo_change))
        .route(
            "/tasks/{id}/attachments",
            get(list_attachments)
                .post(upload_attachment)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/tasks/{id}/attachments/{attachment_id}/download",
            get(download_attachment),
        )
        .route("/tasks/{id}/cover", post(set_cover).delete(clear_cover))
        .route("/attachments/{id}/file", get(public_attachment))
        .route(
            "/tasks/{id}/attachments/{attachment_id}",
            axum::routing::delete(delete_attachment),
        )
        .route("/tasks/{id}/move", post(move_task))
        .route(
            "/tasks/{id}/subtasks",
            get(list_subtasks).post(create_subtask),
        )
        .route("/subtasks/{id}", patch(toggle_subtask))
        .route(
            "/tasks/{id}/comments",
            get(list_comments).post(create_comment),
        )
        .route("/my/tasks", get(my_tasks))
}
