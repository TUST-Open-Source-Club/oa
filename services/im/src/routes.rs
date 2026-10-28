//! HTTP 路由。

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use club_auth_sdk::AuthUser;
use club_common::{AppError, FieldError};

use crate::entity::{conversation, conversation_member, message};
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

/// 解析登录用户 ID。
fn user_id_of(auth: &AuthUser) -> Result<Uuid, AppError> {
    auth.claims()
        .sub
        .parse()
        .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "访问令牌无效"))
}

/// 消息 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    /// ID。
    pub id: String,
    /// 所属会话。
    pub conversation_id: String,
    /// 会话内序号。
    pub seq: i64,
    /// 发送者。
    pub sender_id: Option<String>,
    /// 类型。
    #[serde(rename = "type")]
    pub kind: String,
    /// 内容。
    pub content: Value,
    /// 引用消息。
    pub reply_to_id: Option<String>,
    /// 状态。
    pub status: String,
    /// @ 提及（成员 ID 与 `"@all"`）。
    pub mentions: Vec<String>,
    /// 线程根消息（null = 主消息流）。
    pub thread_root_id: Option<String>,
    /// 线程回复数（仅根消息有意义）。
    pub thread_reply_count: i64,
    /// 表情回应聚合。
    pub reactions: Vec<ReactionDto>,
    /// 当前用户是否收藏。
    pub starred: bool,
    /// 编辑时间。
    pub edited_at: Option<DateTime<FixedOffset>>,
    /// 时间。
    pub created_at: DateTime<FixedOffset>,
}

impl From<&message::Model> for MessageDto {
    fn from(model: &message::Model) -> Self {
        Self {
            id: model.id.to_string(),
            conversation_id: model.conversation_id.to_string(),
            seq: model.seq,
            sender_id: model.sender_id.map(|id| id.to_string()),
            kind: model.r#type.clone(),
            content: model.content.clone(),
            reply_to_id: model.reply_to_id.map(|id| id.to_string()),
            status: model.status.clone(),
            mentions: model
                .mentions
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            thread_root_id: model.thread_root_id.map(|id| id.to_string()),
            thread_reply_count: 0,
            reactions: Vec::new(),
            starred: false,
            edited_at: model.edited_at.map(|t| t.fixed_offset()),
            created_at: model.created_at,
        }
    }
}

/// 表情回应聚合 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionDto {
    /// 表情。
    pub emoji: String,
    /// 回应人数。
    pub count: i64,
    /// 当前用户是否回应过。
    pub mine: bool,
}

/// 把聚合结果转成 DTO（`mine` 按调用者判定）。
fn reaction_dtos(groups: &[repo::ReactionGroup], viewer: Uuid) -> Vec<ReactionDto> {
    groups
        .iter()
        .map(|group| ReactionDto {
            emoji: group.emoji.clone(),
            count: group.count,
            mine: group.user_ids.contains(&viewer),
        })
        .collect()
}

/// 输出用 DTO：附件在**返回时**重新签发下载 URL，避免消息里存的旧 token 过期裂图。
pub fn message_dto(state: &SharedState, model: &message::Model) -> MessageDto {
    let mut dto = MessageDto::from(model);
    if let Some(Value::Array(items)) = model.content.get("attachments") {
        let mut rewritten = Vec::with_capacity(items.len());
        for item in items {
            let mut obj = item.clone();
            if let Some(id) = obj.get("id").and_then(Value::as_str) {
                if let Ok(file_id) = Uuid::parse_str(id) {
                    if let Ok(token) = sign_file_token(&state.config.internal_secret, file_id) {
                        obj["token"] = json!(token);
                        obj["url"] = json!(format!("/api/v1/im/files/{file_id}?token={token}"));
                    }
                }
            }
            rewritten.push(obj);
        }
        dto.content["attachments"] = Value::Array(rewritten);
    }
    dto
}

/// 会话 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDto {
    /// ID。
    pub id: String,
    /// 类型。
    #[serde(rename = "type")]
    pub kind: String,
    /// 名称。
    pub name: Option<String>,
    /// 公告。
    pub notice: Option<String>,
    /// 是否仅管理员可发言。
    pub only_admins_speak: bool,
    /// 未读数。
    pub unread: i64,
    /// 是否有未读的 @ 提及。
    pub has_mention: bool,
    /// 近 5 秒内正在输入的用户（含自己，前端过滤）。
    pub typing_user_ids: Vec<String>,
    /// 是否免打扰。
    pub muted: bool,
    /// 我的通知级别（all / mentions / mute）。
    pub notify_level: String,
    /// 是否置顶。
    pub pinned: bool,
    /// 成员 ID。
    pub member_ids: Vec<String>,
    /// 最近一条消息预览。
    pub last_message: Option<MessageDto>,
    /// 更新时间。
    pub updated_at: DateTime<FixedOffset>,
}

/// `GET /conversations`：会话列表。
pub async fn list_conversations(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> Result<Json<Vec<ConversationDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let rows = repo::list_conversations(&state.db, user_id).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let members = repo::list_members(&state.db, row.conversation.id).await?;
        let last = repo::list_messages(&state.db, row.conversation.id, None, 1).await?;
        items.push(ConversationDto {
            id: row.conversation.id.to_string(),
            kind: row.conversation.r#type.clone(),
            name: row.conversation.name.clone(),
            notice: row.conversation.notice.clone(),
            only_admins_speak: row.conversation.only_admins_speak,
            unread: repo::unread_for(&row.conversation, &row.member),
            has_mention: repo::has_unread_mention(
                &state.db,
                row.conversation.id,
                user_id,
                row.member.last_read_seq,
            )
            .await?,
            muted: row.member.muted,
            notify_level: row.member.notify_level.clone(),
            pinned: row.member.pinned,
            typing_user_ids: state
                .typing
                .active(row.conversation.id, state.now())
                .iter()
                .map(|id| id.to_string())
                .collect(),
            member_ids: members.iter().map(|m| m.user_id.to_string()).collect(),
            last_message: last.first().map(|item| message_dto(&state, item)),
            updated_at: row.conversation.updated_at,
        });
    }
    Ok(Json(items))
}

/// 创建会话请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationRequest {
    /// direct / group。
    #[serde(rename = "type")]
    pub kind: String,
    /// 其他成员。
    pub member_ids: Vec<Uuid>,
    /// 群名（群聊必填）。
    pub name: Option<String>,
}

/// `POST /conversations`：创建单聊/群聊（单聊幂等）。
pub async fn create_conversation(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(input): Json<CreateConversationRequest>,
) -> Result<(StatusCode, Json<ConversationDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    let now = state.now();
    let (conversation, created) = match input.kind.as_str() {
        conversation::TYPE_DIRECT => {
            let other = input.member_ids.first().ok_or_else(|| {
                AppError::unprocessable(
                    "IM_VALIDATION",
                    "单聊需要一个对方用户",
                    vec![FieldError::new("memberIds", "不能为空")],
                )
            })?;
            repo::create_direct_conversation(&state.db, user_id, *other, now).await?
        }
        conversation::TYPE_GROUP => {
            let name = input
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty());
            let Some(name) = name else {
                return Err(AppError::unprocessable(
                    "IM_VALIDATION",
                    "群聊名称不能为空",
                    vec![FieldError::new("name", "不能为空")],
                ));
            };
            if input.member_ids.is_empty() {
                return Err(AppError::unprocessable(
                    "IM_VALIDATION",
                    "群聊至少邀请一位成员",
                    vec![FieldError::new("memberIds", "不能为空")],
                ));
            }
            let conv =
                repo::create_group_conversation(&state.db, user_id, &input.member_ids, name, now)
                    .await?;
            (conv, true)
        }
        _ => {
            return Err(AppError::unprocessable(
                "IM_VALIDATION",
                "会话类型不合法",
                vec![FieldError::new("type", "仅支持 direct / group")],
            ))
        }
    };

    let members = repo::list_members(&state.db, conversation.id).await?;
    let member = repo::find_member(&state.db, conversation.id, user_id)
        .await?
        .ok_or_else(|| AppError::internal("创建会话后成员记录缺失"))?;
    let response = ConversationDto {
        id: conversation.id.to_string(),
        kind: conversation.r#type.clone(),
        name: conversation.name.clone(),
        notice: conversation.notice.clone(),
        only_admins_speak: conversation.only_admins_speak,
        unread: repo::unread_for(&conversation, &member),
        has_mention: false,
        muted: member.muted,
        notify_level: member.notify_level.clone(),
        pinned: member.pinned,
        member_ids: members.iter().map(|m| m.user_id.to_string()).collect(),
        typing_user_ids: Vec::new(),
        last_message: None,
        updated_at: conversation.updated_at,
    };
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(response)))
}

/// 历史消息查询。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageQuery {
    /// 拉取该序号之前（不含）。
    pub before_seq: Option<i64>,
    /// 条数。
    pub limit: Option<u64>,
}

/// `GET /conversations/{id}/messages`：历史消息。
pub async fn list_messages(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<MessageDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let items = repo::list_messages(&state.db, conversation_id, query.before_seq, limit).await?;
    let root_ids: Vec<Uuid> = items.iter().map(|item| item.id).collect();
    let counts = repo::thread_reply_counts(&state.db, conversation_id, &root_ids).await?;
    let reactions = repo::reactions_for_messages(&state.db, &root_ids).await?;
    let starred = repo::starred_message_ids(&state.db, user_id, &root_ids).await?;
    Ok(Json(
        items
            .iter()
            .map(|item| {
                let mut dto = message_dto(&state, item);
                dto.thread_reply_count = counts.get(&item.id).copied().unwrap_or(0);
                dto.reactions = reaction_dtos(
                    reactions.get(&item.id).map(Vec::as_slice).unwrap_or(&[]),
                    user_id,
                );
                dto.starred = starred.contains(&item.id);
                dto
            })
            .collect(),
    ))
}

/// 发送消息请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    /// 客户端幂等 ID。
    pub client_msg_id: Option<String>,
    /// 类型。
    #[serde(rename = "type")]
    pub kind: String,
    /// 内容。
    pub content: Value,
    /// 引用。
    pub reply_to_id: Option<Uuid>,
    /// @ 提及：成员 ID 数组，`"@all"` 表示所有人。
    #[serde(default)]
    pub mentions: Vec<String>,
    /// 线程根消息（在该消息的线程里回复）。
    pub thread_root_id: Option<Uuid>,
}

/// 校验 `mentions`：成员必须在会话内；`"@all"` 仅群主/管理员、且仅群聊可用。
async fn validate_mentions(
    state: &SharedState,
    ctx: &crate::repo::ConversationWithMember,
    mentions: &[String],
) -> Result<Vec<String>, AppError> {
    if mentions.is_empty() {
        return Ok(Vec::new());
    }
    let privileged = matches!(ctx.member.role.as_str(), "owner" | "admin");
    for mention in mentions {
        if mention == "@all" {
            if ctx.conversation.r#type != "group" {
                return Err(AppError::bad_request(
                    "IM_MENTION_ALL_INVALID",
                    "单聊不支持 @所有人",
                ));
            }
            if !privileged {
                return Err(AppError::forbidden(
                    "IM_MENTION_ALL_FORBIDDEN",
                    "只有群主或管理员可以 @所有人",
                ));
            }
            continue;
        }
        let id = Uuid::parse_str(mention)
            .map_err(|_| AppError::bad_request("IM_MENTION_INVALID", "提及的用户不存在"))?;
        if crate::repo::get_conversation_for_member(&state.db, ctx.conversation.id, id)
            .await
            .is_err()
        {
            return Err(AppError::bad_request(
                "IM_MENTION_NOT_MEMBER",
                "只能 @ 会话内的成员",
            ));
        }
    }
    Ok(mentions.to_vec())
}

/// `POST /conversations/{id}/messages`：发送消息（HTTP 通道）。
pub async fn send_message(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<MessageDto>), AppError> {
    let user_id = user_id_of(&auth)?;
    // 群管理校验：禁言 / 仅管理员发言
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    if ctx.conversation.r#type == "group" {
        if let Some(until) = ctx.member.muted_until {
            if until > state.now().fixed_offset() {
                return Err(AppError::forbidden("IM_MUTED", "你已被禁言，暂时无法发言"));
            }
        }
        let privileged = matches!(ctx.member.role.as_str(), "owner" | "admin");
        if ctx.conversation.only_admins_speak && !privileged && !ctx.member.can_speak {
            return Err(AppError::forbidden(
                "IM_READONLY_GROUP",
                "当前群仅管理员可发言",
            ));
        }
    }
    let mentions = validate_mentions(&state, &ctx, &input.mentions).await?;
    let (model, created) = crate::service::create_message(
        &state,
        user_id,
        conversation_id,
        &input.kind,
        &input.content,
        input.reply_to_id,
        input.client_msg_id,
        mentions,
        input.thread_root_id,
    )
    .await?;
    if created {
        // 推送给在线的 WebSocket 客户端
        crate::realtime::broadcast_message(&state, conversation_id, &model).await;
    }
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(message_dto(&state, &model))))
}

/// 已读请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequest {
    /// 已读到该序号（含）。
    pub seq: i64,
}

/// `POST /conversations/{id}/typing`：输入心跳（客户端 2~3 秒一次）。
pub async fn typing_heartbeat(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    state.typing.touch(conversation_id, user_id, state.now());
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /conversations/{id}/read`：上报已读位点。
pub async fn mark_read(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<ReadRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let member = crate::service::apply_read(&state, user_id, conversation_id, input.seq).await?;
    Ok(Json(json!({ "lastReadSeq": member.last_read_seq })))
}

/// 通知级别请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyLevelRequest {
    /// all / mentions / mute。
    pub level: String,
}

/// `POST /conversations/{id}/notify`：设置自己的会话通知级别。
pub async fn set_notify_level(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<NotifyLevelRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    if !crate::domain::NOTIFY_LEVELS.contains(&input.level.as_str()) {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "通知级别不合法",
            vec![FieldError::new("level", "仅支持 all / mentions / mute")],
        ));
    }
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let member =
        repo::set_member_notify_level(&state.db, conversation_id, user_id, &input.level).await?;
    Ok(Json(json!({ "level": member.notify_level })))
}

/// 群管理操作请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteRequest {
    /// 禁言到期时间（null 解除禁言）。
    pub muted_until: Option<DateTime<chrono::FixedOffset>>,
}

/// 发言白名单请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    /// 是否允许发言。
    pub can_speak: bool,
}

/// 群设置请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSettingsRequest {
    /// 仅管理员/白名单可发言。
    pub only_admins_speak: Option<bool>,
    /// 群公告（null 清空）。
    #[serde(default, deserialize_with = "double_option")]
    pub notice: Option<Option<String>>,
}

/// 角色请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRequest {
    /// admin / member。
    pub role: String,
}

/// 转让群主请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRequest {
    /// 新群主。
    pub user_id: Uuid,
}

/// 三态 JSON。
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 校验当前用户为群主/管理员，返回（会话, 成员）。
async fn ensure_group_admin(
    state: &SharedState,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<(conversation::Model, conversation_member::Model), AppError> {
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    if ctx.conversation.r#type != "group" {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "仅群聊支持该操作",
            vec![],
        ));
    }
    if !matches!(ctx.member.role.as_str(), "owner" | "admin") {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN_GROUP",
            "仅群主/管理员可操作",
        ));
    }
    Ok((ctx.conversation, ctx.member))
}

/// `POST /conversations/{id}/members/{uid}/mute`：禁言/解除禁言。
pub async fn mute_member(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, target_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<MuteRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let (conversation, operator) = ensure_group_admin(&state, conversation_id, user_id).await?;
    if operator.role == "admin" && conversation.owner_id == Some(target_id) {
        return Err(AppError::forbidden("IM_FORBIDDEN_GROUP", "不能禁言群主"));
    }
    let member = repo::set_member_mute(
        &state.db,
        conversation_id,
        target_id,
        input
            .muted_until
            .map(|value| value.with_timezone(&chrono::Utc)),
    )
    .await?;
    let event = serde_json::json!({
        "type": "member_muted",
        "payload": { "conversationId": conversation_id, "userId": target_id, "mutedUntil": member.muted_until }
    })
    .to_string();
    state.hub.send_to(target_id, &event);
    Ok(Json(
        json!({ "userId": target_id, "mutedUntil": member.muted_until }),
    ))
}

/// `POST /conversations/{id}/members/{uid}/speak`：发言白名单。
pub async fn set_member_speak(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, target_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SpeakRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    ensure_group_admin(&state, conversation_id, user_id).await?;
    let member =
        repo::set_member_speak(&state.db, conversation_id, target_id, input.can_speak).await?;
    Ok(Json(
        json!({ "userId": target_id, "canSpeak": member.can_speak }),
    ))
}

/// `POST /conversations/{id}/settings`：群设置。
pub async fn update_group_settings(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<GroupSettingsRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let (conversation, _) = ensure_group_admin(&state, conversation_id, user_id).await?;
    let updated = repo::update_group_settings(
        &state.db,
        &conversation,
        input.only_admins_speak,
        input.notice,
        state.now(),
    )
    .await?;
    Ok(Json(json!({
        "onlyAdminsSpeak": updated.only_admins_speak,
        "notice": updated.notice
    })))
}

/// `POST /conversations/{id}/members/{uid}/role`：设置/取消管理员（仅群主）。
pub async fn set_member_role(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, target_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RoleRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let (conversation, operator) = ensure_group_admin(&state, conversation_id, user_id).await?;
    if operator.role != "owner" {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN_GROUP",
            "仅群主可设置管理员",
        ));
    }
    if !["admin", "member"].contains(&input.role.as_str()) {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "角色不合法",
            vec![FieldError::new("role", "仅支持 admin/member")],
        ));
    }
    if conversation.owner_id == Some(target_id) {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN_GROUP",
            "不能修改群主角色",
        ));
    }
    let member = repo::set_member_role(&state.db, conversation_id, target_id, &input.role).await?;
    let members = repo::list_members(&state.db, conversation_id).await?;
    let event = serde_json::json!({
        "type": "my_role_updated",
        "payload": { "conversationId": conversation_id, "role": member.role }
    })
    .to_string();
    state.hub.send_to(target_id, &event);
    let _ = members;
    Ok(Json(json!({ "userId": target_id, "role": member.role })))
}

/// `POST /conversations/{id}/transfer`：转让群主（仅群主）。
pub async fn transfer_owner(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<TransferRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let (conversation, operator) = ensure_group_admin(&state, conversation_id, user_id).await?;
    if operator.role != "owner" {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN_GROUP",
            "仅群主可转让群主",
        ));
    }
    let updated =
        repo::transfer_owner(&state.db, &conversation, input.user_id, state.now()).await?;
    let event = serde_json::json!({
        "type": "owner_transferred",
        "payload": { "conversationId": conversation_id, "ownerId": input.user_id }
    })
    .to_string();
    let members = repo::list_members(&state.db, conversation_id).await?;
    for member in members {
        state.hub.send_to(member.user_id, &event);
    }
    Ok(Json(json!({ "ownerId": updated.owner_id })))
}

/// `DELETE /conversations/{id}`：解散群聊（仅群主）。
pub async fn dissolve_conversation(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let (conversation, operator) = ensure_group_admin(&state, conversation_id, user_id).await?;
    if operator.role != "owner" {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN_GROUP",
            "仅群主可解散群聊",
        ));
    }
    let members = repo::list_members(&state.db, conversation_id).await?;
    repo::dissolve_conversation(&state.db, conversation_id).await?;
    let _ = conversation;
    let event = serde_json::json!({
        "type": "conversation_dissolved",
        "payload": { "conversationId": conversation_id }
    })
    .to_string();
    for member in members {
        state.hub.send_to(member.user_id, &event);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /conversations/{id}/messages/{message_id}/recall`：撤回消息并广播。
pub async fn recall_message(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<MessageDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let model =
        crate::service::recall_message(&state, user_id, conversation_id, message_id).await?;
    let members = repo::list_members(&state.db, conversation_id).await?;
    let event = serde_json::json!({
        "type": "message_recalled",
        "payload": { "conversationId": conversation_id, "messageId": message_id }
    })
    .to_string();
    for member in members {
        state.hub.send_to(member.user_id, &event);
    }
    Ok(Json(message_dto(&state, &model)))
}

/// `GET /conversations/{id}/messages/{message_id}/receipts`：已读回执。
pub async fn message_receipts(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let receipts =
        crate::service::read_receipts(&state, user_id, conversation_id, message_id).await?;
    Ok(Json(json!(receipts)))
}

/// `GET /conversations/{id}/members`：成员列表。
pub async fn list_members(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let members = repo::list_members(&state.db, conversation_id).await?;
    let items: Vec<Value> = members
        .iter()
        .map(|m| {
            json!({
                "userId": m.user_id,
                "role": m.role,
                "lastReadSeq": m.last_read_seq,
                "mutedUntil": m.muted_until,
                "canSpeak": m.can_speak,
                "joinedAt": m.joined_at
            })
        })
        .collect();
    Ok(Json(json!(items)))
}

/// 添加成员请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMembersRequest {
    /// 用户 ID 列表。
    pub user_ids: Vec<Uuid>,
}

/// `POST /conversations/{id}/members`：邀请成员（仅群聊，群主/管理员）。
pub async fn add_members(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<AddMembersRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    if ctx.conversation.r#type != conversation::TYPE_GROUP {
        return Err(AppError::bad_request("IM_NOT_GROUP", "单聊不能添加成员"));
    }
    if !matches!(
        ctx.member.role.as_str(),
        conversation_member::ROLE_OWNER | conversation_member::ROLE_ADMIN
    ) {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN",
            "仅群主/管理员可邀请成员",
        ));
    }
    let now = state.now();
    let mut added = 0;
    for member_id in &input.user_ids {
        repo::insert_member(
            &state.db,
            conversation_id,
            *member_id,
            conversation_member::ROLE_MEMBER,
            now,
        )
        .await?;
        added += 1;
    }
    Ok(Json(json!({ "added": added })))
}

/// `DELETE /conversations/{id}/members/{userId}`：移出成员或主动退群。
pub async fn remove_member(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = user_id_of(&auth)?;
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let is_self = user_id == target_id;
    let can_manage = matches!(
        ctx.member.role.as_str(),
        conversation_member::ROLE_OWNER | conversation_member::ROLE_ADMIN
    );
    if !is_self && !can_manage {
        return Err(AppError::forbidden(
            "IM_FORBIDDEN",
            "仅群主/管理员可移出成员",
        ));
    }
    repo::remove_member(&state.db, conversation_id, target_id, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 单个媒体文件的最大字节数（5MB）。
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
/// 媒体下载签名有效期（秒）。
const FILE_TOKEN_TTL_SECONDS: i64 = 3600;

/// 上传媒体文件请求（base64 编码，避免 multipart 依赖）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileRequest {
    /// 原始文件名。
    pub name: String,
    /// MIME 类型。
    pub mime: String,
    /// 文件内容的 base64。
    pub data_base64: String,
}

/// 媒体下载签名载荷。
#[derive(Debug, Serialize, Deserialize)]
struct FileTokenClaims {
    /// 文件 ID。
    sub: String,
    /// 过期时间戳。
    exp: i64,
}

/// 生成媒体下载签名（HS256，用服务内部密钥）。
fn sign_file_token(secret: &str, file_id: Uuid) -> Result<String, AppError> {
    let claims = FileTokenClaims {
        sub: file_id.to_string(),
        exp: chrono::Utc::now().timestamp() + FILE_TOKEN_TTL_SECONDS,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

/// `POST /uploads`：上传媒体文件（返回可直接放进消息 content 的附件信息）。
pub async fn upload_file(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(input): Json<UploadFileRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    use base64::Engine as _;

    let user_id = user_id_of(&auth)?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "文件名不合法",
            vec![FieldError::new("name", "不能为空且不超过 200 字")],
        ));
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(input.data_base64.trim())
        .map_err(|_| {
            AppError::unprocessable(
                "IM_VALIDATION",
                "文件内容不是合法 base64",
                vec![FieldError::new("dataBase64", "格式错误")],
            )
        })?;
    if data.is_empty() || data.len() > MAX_FILE_BYTES {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "文件大小不合法",
            vec![FieldError::new(
                "dataBase64",
                format!("需在 1 字节到 {}MB 之间", MAX_FILE_BYTES / 1024 / 1024),
            )],
        ));
    }
    let mime = if input.mime.trim().is_empty() {
        "application/octet-stream"
    } else {
        input.mime.trim()
    };
    let model = repo::insert_file(&state.db, user_id, name, mime, data, state.now()).await?;
    let token = sign_file_token(&state.config.internal_secret, model.id)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": model.id,
            "name": model.name,
            "mime": model.mime,
            "size": model.size,
            "token": token,
            "url": format!("/api/v1/im/files/{}?token={}", model.id, token),
        })),
    ))
}

/// 媒体下载查询参数。
#[derive(Debug, Deserialize)]
pub struct FileTokenQuery {
    /// 签名。
    pub token: String,
}

/// `GET /files/{id}?token=`：按签名下载媒体（`<img>` 可直接引用）。
pub async fn download_file(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(query): Query<FileTokenQuery>,
) -> Result<Response, AppError> {
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    let decoded = jsonwebtoken::decode::<FileTokenClaims>(
        &query.token,
        &jsonwebtoken::DecodingKey::from_secret(state.config.internal_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::unauthorized("AUTH_INVALID_TOKEN", "下载链接无效或已过期"))?;
    if decoded.claims.sub != id.to_string() {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_TOKEN",
            "下载链接与文件不匹配",
        ));
    }
    let file = repo::find_file(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_FILE_NOT_FOUND", "文件不存在"))?;
    let filename = file.name.replace(['"', '\n', '\r'], "");
    Ok((
        [
            (header::CONTENT_TYPE, file.mime),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
        ],
        file.data,
    )
        .into_response())
}

/// 导出查询参数。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    /// md（默认）/ json。
    pub format: Option<String>,
}

/// 导出消息上限。
pub const MAX_EXPORT_MESSAGES: u64 = 20_000;

/// 消息在导出文本里的正文摘要。
fn export_body(message: &message::Model) -> String {
    if message.status == "recalled" {
        return "（已撤回）".to_string();
    }
    match message.r#type.as_str() {
        "text" => message
            .content
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        "image" => format!("[图片] {}", attachment_names(message)),
        "file" => format!("[文件] {}", attachment_names(message)),
        other => format!("[{other}]"),
    }
}

/// 附件文件名（逗号分隔）。
fn attachment_names(message: &message::Model) -> String {
    message
        .content
        .get("attachments")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default()
}

/// 渲染 Markdown 导出（线程回复缩进显示在根消息后）。
fn render_markdown(conversation: &conversation::Model, messages: &[message::Model]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# {}\n\n",
        conversation.name.as_deref().unwrap_or("私聊")
    ));
    out.push_str(&format!(
        "> 导出时间：{} · 消息数：{}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        messages.len()
    ));
    for root in messages.iter().filter(|item| item.thread_root_id.is_none()) {
        push_markdown_message(&mut out, root, 0);
        for reply in messages
            .iter()
            .filter(|item| item.thread_root_id == Some(root.id))
        {
            push_markdown_message(&mut out, reply, 1);
        }
    }
    out
}

/// 追加一条消息到 Markdown。
fn push_markdown_message(out: &mut String, message: &message::Model, depth: usize) {
    let indent = if depth == 0 { "" } else { "  > " };
    let prefix = if depth == 0 { "###" } else { ">" };
    let edited = if message.edited_at.is_some() {
        "（已编辑）"
    } else {
        ""
    };
    out.push_str(&format!(
        "{indent}{prefix} {} · `{}` · {}{edited}\n{indent}\n",
        message.created_at.format("%Y-%m-%d %H:%M"),
        message
            .sender_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "系统".to_string()),
        message.r#type,
    ));
    for line in export_body(message).lines() {
        out.push_str(&format!("{indent}{line}\n"));
    }
    out.push('\n');
}

/// `GET /conversations/{id}/export?format=md|json`：导出会话（成员可导出，写审计）。
pub async fn export_conversation(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let user_id = user_id_of(&auth)?;
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let format = query.format.as_deref().unwrap_or("md");
    if !matches!(format, "md" | "json") {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "导出格式不支持",
            vec![FieldError::new("format", "仅支持 md / json")],
        ));
    }
    let messages = repo::list_all_messages(&state.db, conversation_id, MAX_EXPORT_MESSAGES).await?;
    let (content_type, body, extension) = if format == "md" {
        (
            "text/markdown; charset=utf-8",
            render_markdown(&ctx.conversation, &messages),
            "md",
        )
    } else {
        let payload = json!({
            "conversation": {
                "id": ctx.conversation.id,
                "type": ctx.conversation.r#type,
                "name": ctx.conversation.name,
                "notice": ctx.conversation.notice,
                "createdAt": ctx.conversation.created_at,
            },
            "messages": messages.iter().map(|item| message_dto(&state, item)).collect::<Vec<_>>(),
        });
        let text = serde_json::to_string_pretty(&payload).map_err(AppError::internal)?;
        ("application/json; charset=utf-8", text, "json")
    };
    if let Err(err) = club_bus::audit::record(
        &state.db,
        "im_conversation",
        &conversation_id.to_string(),
        "export",
        None,
        Some(json!({ "format": format, "messageCount": messages.len() })),
        Some(user_id),
        state.now(),
    )
    .await
    {
        tracing::warn!(error = %err, "写导出审计失败");
    }
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"conversation-{conversation_id}.{extension}\""),
            ),
        ],
        body,
    )
        .into_response())
}

/// 置顶请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRequest {
    /// 是否置顶。
    pub pinned: bool,
}

/// `POST /conversations/{id}/pin`：设置自己的会话置顶。
pub async fn set_pin(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<PinRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let member = repo::set_member_pinned(&state.db, conversation_id, user_id, input.pinned).await?;
    Ok(Json(json!({ "pinned": member.pinned })))
}

/// `POST /conversations/{id}/messages/{mid}/star`：切换收藏。
pub async fn toggle_star(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    let starred =
        repo::toggle_star(&state.db, user_id, conversation_id, message_id, state.now()).await?;
    Ok(Json(json!({ "starred": starred })))
}

/// `GET /messages/starred`：我收藏的消息。
pub async fn list_starred(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<MessageDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let items = repo::list_starred(&state.db, user_id, limit).await?;
    Ok(Json(
        items
            .iter()
            .map(|item| {
                let mut dto = message_dto(&state, item);
                dto.starred = true;
                dto
            })
            .collect(),
    ))
}

/// 编辑消息请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditMessageRequest {
    /// 新的正文（类型沿用原消息）。
    pub content: Value,
}

/// 编辑历史版本 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionDto {
    /// 版本号（1 为首次编辑前补录的原内容）。
    pub version: i32,
    /// 该版正文。
    pub content: Value,
    /// 编辑者。
    pub editor_id: Option<String>,
    /// 该版本产生时间。
    pub created_at: DateTime<FixedOffset>,
}

impl From<&crate::entity::message_revision::Model> for RevisionDto {
    fn from(model: &crate::entity::message_revision::Model) -> Self {
        Self {
            version: model.version,
            content: model.content.clone(),
            editor_id: model.editor_id.map(|id| id.to_string()),
            created_at: model.created_at,
        }
    }
}

/// `PATCH /conversations/{id}/messages/{message_id}`：编辑自己的消息并广播。
pub async fn edit_message(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<EditMessageRequest>,
) -> Result<Json<MessageDto>, AppError> {
    let user_id = user_id_of(&auth)?;
    let (model, edited) =
        crate::service::edit_message(&state, user_id, conversation_id, message_id, &input.content)
            .await?;
    if edited {
        crate::realtime::broadcast_edited(&state, conversation_id, &model).await;
    }
    Ok(Json(message_dto(&state, &model)))
}

/// `GET /conversations/{id}/messages/{message_id}/revisions`：编辑历史（版本倒序）。
pub async fn message_revisions(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<RevisionDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let revisions =
        crate::service::message_revisions(&state, user_id, conversation_id, message_id).await?;
    Ok(Json(revisions.iter().map(RevisionDto::from).collect()))
}

/// `GET /conversations/{id}/messages/{message_id}/thread`：线程回复（按 seq 升序）。
pub async fn list_thread(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<MessageDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    let replies = repo::list_thread_messages(&state.db, conversation_id, message_id).await?;
    let ids: Vec<Uuid> = replies.iter().map(|item| item.id).collect();
    let reactions = repo::reactions_for_messages(&state.db, &ids).await?;
    let starred = repo::starred_message_ids(&state.db, user_id, &ids).await?;
    Ok(Json(
        replies
            .iter()
            .map(|item| {
                let mut dto = message_dto(&state, item);
                dto.reactions = reaction_dtos(
                    reactions.get(&item.id).map(Vec::as_slice).unwrap_or(&[]),
                    user_id,
                );
                dto.starred = starred.contains(&item.id);
                dto
            })
            .collect(),
    ))
}

/// 表情回应请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionRequest {
    /// 表情。
    pub emoji: String,
}

/// `POST /conversations/{id}/messages/{message_id}/reactions`：切换表情回应。
pub async fn toggle_reaction(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReactionRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = user_id_of(&auth)?;
    let groups =
        crate::service::toggle_reaction(&state, user_id, conversation_id, message_id, &input.emoji)
            .await?;
    crate::realtime::broadcast_reactions(&state, conversation_id, message_id, &groups).await;
    Ok(Json(
        json!({ "reactions": reaction_dtos(&groups, user_id) }),
    ))
}

/// 搜索参数。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    /// 关键词。
    pub q: String,
    /// 条数。
    pub limit: Option<u64>,
}

/// `GET /conversations/{id}/messages/search`：会话内搜索。
pub async fn search_in_conversation(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<MessageDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let items =
        crate::service::search_messages(&state, &[conversation_id], &query.q, query.limit).await?;
    Ok(Json(
        items.iter().map(|item| message_dto(&state, item)).collect(),
    ))
}

/// `GET /search/messages`：跨会话搜索（仅自己参与的会话）。
pub async fn search_messages(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<MessageDto>>, AppError> {
    let user_id = user_id_of(&auth)?;
    let conversations = repo::list_conversations(&state.db, user_id).await?;
    let ids: Vec<Uuid> = conversations
        .iter()
        .map(|row| row.conversation.id)
        .collect();
    let items = crate::service::search_messages(&state, &ids, &query.q, query.limit).await?;
    Ok(Json(
        items.iter().map(|item| message_dto(&state, item)).collect(),
    ))
}

/// `/api/v1/im` 路由。
pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/conversations/{id}/messages",
            get(list_messages).post(send_message),
        )
        .route(
            "/conversations/{id}/messages/{message_id}",
            patch(edit_message),
        )
        .route(
            "/conversations/{id}/messages/{message_id}/revisions",
            get(message_revisions),
        )
        .route(
            "/conversations/{id}/messages/{message_id}/thread",
            get(list_thread),
        )
        .route(
            "/conversations/{id}/messages/{message_id}/reactions",
            post(toggle_reaction),
        )
        .route(
            "/conversations/{id}/messages/{message_id}/recall",
            post(recall_message),
        )
        .route(
            "/conversations/{id}/messages/{message_id}/receipts",
            get(message_receipts),
        )
        .route(
            "/conversations/{id}/members",
            get(list_members).post(add_members),
        )
        .route(
            "/conversations/{id}/members/{user_id}",
            delete(remove_member),
        )
        .route(
            "/conversations/{id}/messages/search",
            get(search_in_conversation),
        )
        .route("/search/messages", get(search_messages))
        .route("/conversations/{id}/pin", post(set_pin))
        .route(
            "/conversations/{id}/messages/{message_id}/star",
            post(toggle_star),
        )
        .route("/messages/starred", get(list_starred))
        .route("/conversations/{id}/export", get(export_conversation))
        .route("/uploads", post(upload_file))
        .route("/files/{id}", get(download_file))
        .route("/conversations/{id}/notify", post(set_notify_level))
        .route("/conversations/{id}/typing", post(typing_heartbeat))
        .route("/conversations/{id}/read", post(mark_read))
        .route(
            "/conversations/{id}/members/{user_id}/mute",
            post(mute_member),
        )
        .route(
            "/conversations/{id}/members/{user_id}/speak",
            post(set_member_speak),
        )
        .route("/conversations/{id}/settings", post(update_group_settings))
        .route(
            "/conversations/{id}/members/{user_id}/role",
            post(set_member_role),
        )
        .route("/conversations/{id}/transfer", post(transfer_owner))
        .route(
            "/conversations/{id}",
            axum::routing::delete(dissolve_conversation),
        )
}
