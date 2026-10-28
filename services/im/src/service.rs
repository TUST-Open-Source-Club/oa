//! 消息领域服务：HTTP 与 WebSocket 共用的发送/已读/撤回逻辑。

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use club_common::{AppError, FieldError};

use crate::domain;
use crate::entity::{conversation, conversation_member, message};
use crate::repo;
use crate::state::SharedState;

/// 撤回时限（秒）：发送后 2 分钟内本人可撤回。
pub const RECALL_WINDOW_SECONDS: i64 = 120;

/// 发送消息（幂等），返回消息与是否新建；新建时写 outbox 事件。
// 发消息参数较多（类型/内容/引用/幂等/提及），平铺比再包一层结构体更直观。
#[allow(clippy::too_many_arguments)]
pub async fn create_message(
    state: &SharedState,
    sender_id: Uuid,
    conversation_id: Uuid,
    kind: &str,
    content: &Value,
    reply_to_id: Option<Uuid>,
    client_msg_id: Option<String>,
    mentions: Vec<String>,
    thread_root_id: Option<Uuid>,
) -> Result<(message::Model, bool), AppError> {
    domain::validate_message(kind, content)?;
    let ctx = repo::get_conversation_for_member(&state.db, conversation_id, sender_id).await?;
    if let Some(reply_to_id) = reply_to_id {
        if repo::find_message(&state.db, conversation_id, reply_to_id)
            .await?
            .is_none()
        {
            return Err(AppError::unprocessable(
                "IM_VALIDATION",
                "引用的消息不存在",
                vec![FieldError::new("replyToId", "消息不存在")],
            ));
        }
    }

    // 线程根必须是同会话的主消息流消息（不再嵌套线程）
    if let Some(root_id) = thread_root_id {
        let root = repo::find_message(&state.db, conversation_id, root_id)
            .await?
            .ok_or_else(|| {
                AppError::unprocessable(
                    "IM_VALIDATION",
                    "线程根消息不存在",
                    vec![FieldError::new("threadRootId", "消息不存在")],
                )
            })?;
        if root.thread_root_id.is_some() {
            return Err(AppError::bad_request(
                "IM_THREAD_NESTED",
                "线程内不能再开线程",
            ));
        }
    }

    let now = state.now();
    let (model, created) = repo::insert_message(
        &state.db,
        repo::NewMessage {
            conversation_id,
            sender_id,
            kind: kind.to_string(),
            content: content.clone(),
            reply_to_id,
            client_msg_id,
            mentions: mentions.clone(),
            thread_root_id,
        },
        now,
    )
    .await?;

    if created && state.bus.is_some() {
        let members = repo::list_members(&state.db, conversation_id).await?;
        // 按每位成员的会话通知级别裁剪提醒对象（静音 / 仅 @我）
        let targets: Vec<String> = members
            .iter()
            .filter(|member| member.user_id != sender_id)
            .filter(|member| domain::should_notify(&member.notify_level, member.user_id, &mentions))
            .map(|member| member.user_id.to_string())
            .collect();
        let title = match ctx.conversation.r#type.as_str() {
            conversation::TYPE_GROUP => ctx
                .conversation
                .name
                .clone()
                .unwrap_or_else(|| "群聊".to_string()),
            _ => "新消息".to_string(),
        };
        let payload = json!({
            "id": model.id,
            "type": "im.message.created",
            "actorId": sender_id,
            "targetUsers": targets,
            "resource": { "type": "im", "id": conversation_id, "url": format!("/im/{conversation_id}") },
            "title": title,
            "body": domain::preview_of(&model.r#type, &model.content),
            // 供 notify 判断是否要发「@了你 / @所有人」提醒
            "mentions": mentions,
            "priority": "high"
        });
        if let Err(err) =
            club_bus::outbox::enqueue(&state.db, "im.message.created", &payload, now).await
        {
            tracing::warn!(error = %err, "消息事件写入 outbox 失败");
        }
    }
    Ok((model, created))
}

/// 更新已读位点并返回成员记录。
pub async fn apply_read(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    seq: i64,
) -> Result<conversation_member::Model, AppError> {
    repo::update_last_read(&state.db, conversation_id, user_id, seq).await
}

/// 撤回消息（仅发送者、限时 2 分钟；重复撤回幂等返回）。
pub async fn recall_message(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
) -> Result<message::Model, AppError> {
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let model = repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    if model.sender_id != Some(user_id) {
        return Err(AppError::forbidden("IM_FORBIDDEN", "只能撤回自己的消息"));
    }
    if model.status == "recalled" {
        // 本人重复撤回：幂等返回
        return Ok(model);
    }
    let now = Utc::now();
    let age = now.signed_duration_since(model.created_at.with_timezone(&Utc));
    if age.num_seconds() > RECALL_WINDOW_SECONDS {
        return Err(AppError::forbidden(
            "IM_RECALL_EXPIRED",
            "超过 2 分钟的消息不能撤回",
        ));
    }
    repo::mark_recalled(&state.db, &model, now).await
}

/// 已读回执：统计已读到某消息的成员。
pub async fn read_receipts(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
) -> Result<Vec<Value>, AppError> {
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let target = repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    let members = repo::list_members(&state.db, conversation_id).await?;
    Ok(members
        .iter()
        .filter(|member| member.user_id != user_id)
        .map(|member| {
            json!({
                "userId": member.user_id,
                "read": member.last_read_seq >= target.seq,
                "lastReadSeq": member.last_read_seq
            })
        })
        .collect())
}

/// 编辑消息（仅发送者本人，返回消息与是否实际改动）。
///
/// 内容与当前一致时按幂等处理：不写版本历史、不更新 `edited_at`。
pub async fn edit_message(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
    content: &Value,
) -> Result<(message::Model, bool), AppError> {
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let model = repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    if model.sender_id != Some(user_id) {
        return Err(AppError::forbidden("IM_FORBIDDEN", "只能编辑自己的消息"));
    }
    if model.status == "recalled" {
        return Err(AppError::bad_request(
            "IM_MESSAGE_RECALLED",
            "已撤回的消息不能编辑",
        ));
    }
    domain::validate_message(&model.r#type, content)?;
    if &model.content == content {
        return Ok((model, false));
    }
    let updated = repo::edit_message_content(
        &state.db,
        conversation_id,
        message_id,
        content.clone(),
        user_id,
        state.now(),
    )
    .await?;
    Ok((updated, true))
}

/// 读取消息编辑历史（仅会话成员）。
pub async fn message_revisions(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
) -> Result<Vec<crate::entity::message_revision::Model>, AppError> {
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    repo::list_revisions(&state.db, message_id).await
}

/// 表情的最大字节数（含变体选择符，够用即可）。
pub const MAX_EMOJI_BYTES: usize = 32;

/// 切换表情回应，返回该消息的最新聚合。
pub async fn toggle_reaction(
    state: &SharedState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
    emoji: &str,
) -> Result<Vec<repo::ReactionGroup>, AppError> {
    let emoji = emoji.trim();
    if emoji.is_empty() || emoji.len() > MAX_EMOJI_BYTES || emoji.chars().any(char::is_whitespace) {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "表情不合法",
            vec![FieldError::new("emoji", "不能为空且不能含空白")],
        ));
    }
    repo::get_conversation_for_member(&state.db, conversation_id, user_id).await?;
    let model = repo::find_message(&state.db, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    if model.status == "recalled" {
        return Err(AppError::bad_request(
            "IM_MESSAGE_RECALLED",
            "已撤回的消息不能回应",
        ));
    }
    repo::toggle_reaction(&state.db, message_id, user_id, emoji, state.now()).await?;
    let mut grouped = repo::reactions_for_messages(&state.db, &[message_id]).await?;
    Ok(grouped.remove(&message_id).unwrap_or_default())
}

/// 搜索关键词的最大长度。
pub const MAX_SEARCH_QUERY_CHARS: usize = 100;
/// 搜索结果默认与最大条数。
pub const DEFAULT_SEARCH_LIMIT: u64 = 20;
/// 搜索结果上限。
pub const MAX_SEARCH_LIMIT: u64 = 50;

/// 在指定会话集合中搜索消息（调用方负责成员校验）。
pub async fn search_messages(
    state: &SharedState,
    conversation_ids: &[Uuid],
    query: &str,
    limit: Option<u64>,
) -> Result<Vec<message::Model>, AppError> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(AppError::unprocessable(
            "IM_VALIDATION",
            "搜索关键词不合法",
            vec![FieldError::new(
                "q",
                format!("不能为空，且不超过 {MAX_SEARCH_QUERY_CHARS} 字"),
            )],
        ));
    }
    let limit = limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);
    repo::search_messages(&state.db, conversation_ids, query, limit).await
}
