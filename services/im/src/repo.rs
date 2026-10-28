//! 数据访问层：只操作 im schema。

use chrono::{DateTime, Utc};
use sea_orm::sea_query::{Expr, Value as SqlValue};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DbErr, EntityTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;

use club_common::{new_id, AppError};

use crate::domain::unread_of;
use crate::entity::{
    conversation, conversation_member, message, message_reaction, message_revision,
};

/// 将数据库错误映射为统一错误。
pub fn map_db_err(err: DbErr) -> AppError {
    AppError::internal(err)
}

/// 会话 + 当前用户成员关系。
#[derive(Debug, Clone)]
pub struct ConversationWithMember {
    /// 会话。
    pub conversation: conversation::Model,
    /// 当前用户成员记录。
    pub member: conversation_member::Model,
}

/// 创建（或复用）单聊会话。
///
/// 通过扫描自己参与的单聊会话判断是否已有对方，存在则直接返回。
pub async fn create_direct_conversation(
    db: &DatabaseConnection,
    self_id: Uuid,
    other_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(conversation::Model, bool), AppError> {
    if self_id == other_id {
        return Err(AppError::bad_request("IM_SELF_CHAT", "不能与自己创建单聊"));
    }
    // 找自己参与的单聊
    let my_direct_ids: Vec<Uuid> = conversation_member::Entity::find()
        .filter(conversation_member::Column::UserId.eq(self_id))
        .filter(conversation_member::Column::LeftAt.is_null())
        .all(db)
        .await
        .map_err(map_db_err)?
        .into_iter()
        .map(|member| member.conversation_id)
        .collect();
    if !my_direct_ids.is_empty() {
        let direct_convs = conversation::Entity::find()
            .filter(conversation::Column::Id.is_in(my_direct_ids))
            .filter(conversation::Column::Type.eq(conversation::TYPE_DIRECT))
            .all(db)
            .await
            .map_err(map_db_err)?;
        let direct_ids: Vec<Uuid> = direct_convs.iter().map(|conv| conv.id).collect();
        if !direct_ids.is_empty() {
            // 在候选单聊中查找对方也是活跃成员的会话
            let other_memberships = conversation_member::Entity::find()
                .filter(conversation_member::Column::ConversationId.is_in(direct_ids))
                .filter(conversation_member::Column::UserId.eq(other_id))
                .filter(conversation_member::Column::LeftAt.is_null())
                .all(db)
                .await
                .map_err(map_db_err)?;
            if let Some(membership) = other_memberships.first() {
                if let Some(existing) = direct_convs
                    .into_iter()
                    .find(|conv| conv.id == membership.conversation_id)
                {
                    return Ok((existing, false));
                }
            }
        }
    }

    let conversation = conversation::ActiveModel {
        id: Set(new_id()),
        r#type: Set(conversation::TYPE_DIRECT.to_string()),
        name: Set(None),
        owner_id: Set(None),
        notice: Set(None),
        next_seq: Set(0),
        only_admins_speak: Set(false),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;

    for user_id in [self_id, other_id] {
        insert_member(
            db,
            conversation.id,
            user_id,
            conversation_member::ROLE_MEMBER,
            now,
        )
        .await?;
    }
    Ok((conversation, true))
}

/// 创建群聊。
pub async fn create_group_conversation(
    db: &DatabaseConnection,
    owner_id: Uuid,
    member_ids: &[Uuid],
    name: &str,
    now: DateTime<Utc>,
) -> Result<conversation::Model, AppError> {
    let conversation = conversation::ActiveModel {
        id: Set(new_id()),
        r#type: Set(conversation::TYPE_GROUP.to_string()),
        name: Set(Some(name.to_string())),
        owner_id: Set(Some(owner_id)),
        notice: Set(None),
        next_seq: Set(0),
        only_admins_speak: Set(false),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;

    insert_member(
        db,
        conversation.id,
        owner_id,
        conversation_member::ROLE_OWNER,
        now,
    )
    .await?;
    for member_id in member_ids {
        if *member_id != owner_id {
            insert_member(
                db,
                conversation.id,
                *member_id,
                conversation_member::ROLE_MEMBER,
                now,
            )
            .await?;
        }
    }
    Ok(conversation)
}

/// 写入成员（幂等：重复加入时重置 left_at）。
pub async fn insert_member(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    role: &str,
    now: DateTime<Utc>,
) -> Result<conversation_member::Model, AppError> {
    if let Some(existing) = conversation_member::Entity::find_by_id((conversation_id, user_id))
        .one(db)
        .await
        .map_err(map_db_err)?
    {
        let mut active: conversation_member::ActiveModel = existing.into();
        active.left_at = Set(None);
        active.role = Set(role.to_string());
        return active.update(db).await.map_err(map_db_err);
    }
    conversation_member::ActiveModel {
        conversation_id: Set(conversation_id),
        user_id: Set(user_id),
        role: Set(role.to_string()),
        last_read_seq: Set(0),
        muted_until: Set(None),
        can_speak: Set(true),
        muted: Set(false),
        notify_level: Set("all".to_string()),
        pinned: Set(false),
        joined_at: Set(now.fixed_offset()),
        left_at: Set(None),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 查询活跃成员记录。
pub async fn find_member(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<Option<conversation_member::Model>, AppError> {
    conversation_member::Entity::find_by_id((conversation_id, user_id))
        .filter(conversation_member::Column::LeftAt.is_null())
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 校验访问并返回会话 + 成员。
pub async fn get_conversation_for_member(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<ConversationWithMember, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::forbidden("IM_NOT_MEMBER", "你不是该会话成员"))?;
    let conversation = conversation::Entity::find_by_id(conversation_id)
        .one(db)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| AppError::not_found("IM_CONVERSATION_NOT_FOUND", "会话不存在"))?;
    Ok(ConversationWithMember {
        conversation,
        member,
    })
}

/// 列出用户参与的全部会话。
pub async fn list_conversations(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<ConversationWithMember>, AppError> {
    let members = conversation_member::Entity::find()
        .filter(conversation_member::Column::UserId.eq(user_id))
        .filter(conversation_member::Column::LeftAt.is_null())
        .all(db)
        .await
        .map_err(map_db_err)?;
    let mut result = Vec::with_capacity(members.len());
    for member in members {
        if let Some(conversation) = conversation::Entity::find_by_id(member.conversation_id)
            .one(db)
            .await
            .map_err(map_db_err)?
        {
            result.push(ConversationWithMember {
                conversation,
                member,
            });
        }
    }
    result.sort_by_key(|row| std::cmp::Reverse(row.conversation.updated_at));
    Ok(result)
}

/// 列出会话成员（活跃）。
pub async fn list_members(
    db: &DatabaseConnection,
    conversation_id: Uuid,
) -> Result<Vec<conversation_member::Model>, AppError> {
    conversation_member::Entity::find()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .filter(conversation_member::Column::LeftAt.is_null())
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 将成员移出会话。
pub async fn remove_member(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, AppError> {
    let result = conversation_member::Entity::update_many()
        .col_expr(
            conversation_member::Column::LeftAt,
            Expr::value(now.fixed_offset()),
        )
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .filter(conversation_member::Column::UserId.eq(user_id))
        .filter(conversation_member::Column::LeftAt.is_null())
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(result.rows_affected > 0)
}

/// 原子分配会话内下一个消息序号并返回。
pub async fn allocate_seq(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    now: DateTime<Utc>,
) -> Result<i64, AppError> {
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE conversations SET next_seq = next_seq + 1, updated_at = $2 WHERE id = $1 RETURNING next_seq",
        vec![conversation_id.into(), now.fixed_offset().into()],
    );
    let row = db
        .query_one_raw(statement)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| AppError::not_found("IM_CONVERSATION_NOT_FOUND", "会话不存在"))?;
    row.try_get_by_index(0).map_err(map_db_err)
}

/// 发送消息的输入。
#[derive(Debug, Clone)]
pub struct NewMessage {
    /// 会话。
    pub conversation_id: Uuid,
    /// 发送者。
    pub sender_id: Uuid,
    /// 类型。
    pub kind: String,
    /// 内容。
    pub content: Value,
    /// 引用。
    pub reply_to_id: Option<Uuid>,
    /// 客户端幂等 ID。
    pub client_msg_id: Option<String>,
    /// @ 提及（成员 ID 与 `"@all"`）。
    pub mentions: Vec<String>,
    /// 线程根消息（None = 主消息流）。
    pub thread_root_id: Option<Uuid>,
}

/// 插入消息（clientMsgId 幂等：重复发送返回既有消息与 `false`）。
pub async fn insert_message(
    db: &DatabaseConnection,
    new: NewMessage,
    now: DateTime<Utc>,
) -> Result<(message::Model, bool), AppError> {
    if let Some(client_msg_id) = new.client_msg_id.as_deref() {
        if let Some(existing) = message::Entity::find()
            .filter(message::Column::ConversationId.eq(new.conversation_id))
            .filter(message::Column::SenderId.eq(new.sender_id))
            .filter(message::Column::ClientMsgId.eq(client_msg_id))
            .one(db)
            .await
            .map_err(map_db_err)?
        {
            return Ok((existing, false));
        }
    }
    let seq = allocate_seq(db, new.conversation_id, now).await?;
    let model = message::ActiveModel {
        id: Set(new_id()),
        conversation_id: Set(new.conversation_id),
        seq: Set(seq),
        sender_id: Set(Some(new.sender_id)),
        r#type: Set(new.kind),
        content: Set(new.content),
        reply_to_id: Set(new.reply_to_id),
        client_msg_id: Set(new.client_msg_id),
        mentions: Set(serde_json::json!(new.mentions)),
        thread_root_id: Set(new.thread_root_id),
        edited_at: Set(None),
        status: Set("normal".to_string()),
        recalled_at: Set(None),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    Ok((model, true))
}

/// 会话中是否存在未读的 @ 提及（`seq` 大于已读位点且 `mentions` 含该用户）。
pub async fn has_unread_mention(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    after_seq: i64,
) -> Result<bool, AppError> {
    // 未读消息量有界（未读位点之后），这里在应用层判断 jsonb 数组，避免依赖 SQL 方言细节
    let unread = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::Seq.gt(after_seq))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let needle = user_id.to_string();
    Ok(unread.iter().any(|row| {
        row.mentions
            .as_array()
            .map(|items| items.iter().any(|v| v.as_str() == Some(needle.as_str())))
            .unwrap_or(false)
    }))
}

/// 按序号倒序分页拉取历史消息（返回按 seq 升序）。
pub async fn list_messages(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    before_seq: Option<i64>,
    limit: u64,
) -> Result<Vec<message::Model>, AppError> {
    // 主消息流不包含线程回复（线程回复通过 list_thread_messages 拉取）
    let mut select = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::ThreadRootId.is_null())
        .order_by_desc(message::Column::Seq)
        .limit(limit);
    if let Some(before_seq) = before_seq {
        select = select.filter(message::Column::Seq.lt(before_seq));
    }
    let mut items = select.all(db).await.map_err(map_db_err)?;
    items.reverse();
    Ok(items)
}

/// 更新已读位点（只前进不回退）。
pub async fn update_last_read(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    seq: i64,
) -> Result<conversation_member::Model, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::forbidden("IM_NOT_MEMBER", "你不是该会话成员"))?;
    if seq <= member.last_read_seq {
        return Ok(member);
    }
    let mut active: conversation_member::ActiveModel = member.into();
    active.last_read_seq = Set(seq);
    active.update(db).await.map_err(map_db_err)
}

/// 未读数。
pub fn unread_for(conversation: &conversation::Model, member: &conversation_member::Model) -> i64 {
    unread_of(conversation.next_seq, member.last_read_seq)
}

/// 标记消息已撤回。
pub async fn mark_recalled(
    db: &DatabaseConnection,
    model: &message::Model,
    now: DateTime<Utc>,
) -> Result<message::Model, AppError> {
    let mut active: message::ActiveModel = model.clone().into();
    active.status = Set("recalled".to_string());
    active.recalled_at = Set(Some(now.fixed_offset()));
    active.update(db).await.map_err(map_db_err)
}

/// 查询某会话某序号的消息是否存在（引用校验用）。
pub async fn find_message(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    message_id: Uuid,
) -> Result<Option<message::Model>, AppError> {
    message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::Id.eq(message_id))
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 条件查询辅助（保留：按发送者统计等）。
#[allow(dead_code)]
pub fn sender_condition(sender_id: Uuid) -> Condition {
    Condition::all().add(message::Column::SenderId.eq(sender_id))
}

/// 设置成员禁言到期时间。
pub async fn set_member_mute(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    muted_until: Option<DateTime<Utc>>,
) -> Result<conversation_member::Model, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MEMBER_NOT_FOUND", "成员不存在"))?;
    let mut active: conversation_member::ActiveModel = member.into();
    active.muted_until = Set(muted_until.map(|value| value.fixed_offset()));
    active.update(db).await.map_err(map_db_err)
}

/// 设置成员发言白名单。
pub async fn set_member_speak(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    can_speak: bool,
) -> Result<conversation_member::Model, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MEMBER_NOT_FOUND", "成员不存在"))?;
    let mut active: conversation_member::ActiveModel = member.into();
    active.can_speak = Set(can_speak);
    active.update(db).await.map_err(map_db_err)
}

/// 设置成员角色（owner/admin/member）。
pub async fn set_member_role(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<conversation_member::Model, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MEMBER_NOT_FOUND", "成员不存在"))?;
    let mut active: conversation_member::ActiveModel = member.into();
    active.role = Set(role.to_string());
    active.update(db).await.map_err(map_db_err)
}

/// 更新群设置（仅管理员发言/公告）。
pub async fn update_group_settings(
    db: &DatabaseConnection,
    conversation: &conversation::Model,
    only_admins_speak: Option<bool>,
    notice: Option<Option<String>>,
    now: DateTime<Utc>,
) -> Result<conversation::Model, AppError> {
    let mut active: conversation::ActiveModel = conversation.clone().into();
    if let Some(value) = only_admins_speak {
        active.only_admins_speak = Set(value);
    }
    if let Some(value) = notice {
        active.notice = Set(value);
    }
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 转让群主：原群主降为管理员，新群主升为 owner。
pub async fn transfer_owner(
    db: &DatabaseConnection,
    conversation: &conversation::Model,
    new_owner: Uuid,
    now: DateTime<Utc>,
) -> Result<conversation::Model, AppError> {
    if let Some(old_owner) = conversation.owner_id {
        if old_owner == new_owner {
            return Ok(conversation.clone());
        }
        set_member_role(db, conversation.id, old_owner, "admin").await?;
    }
    set_member_role(db, conversation.id, new_owner, "owner").await?;
    let mut active: conversation::ActiveModel = conversation.clone().into();
    active.owner_id = Set(Some(new_owner));
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 解散群聊：删除消息与成员关系，然后删除会话。
pub async fn dissolve_conversation(
    db: &DatabaseConnection,
    conversation_id: Uuid,
) -> Result<(), AppError> {
    use crate::entity::message;
    message::Entity::delete_many()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    conversation_member::Entity::delete_many()
        .filter(conversation_member::Column::ConversationId.eq(conversation_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    conversation::Entity::delete_by_id(conversation_id)
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(())
}

/// 列出某条消息的编辑历史（版本倒序）。
pub async fn list_revisions(
    db: &DatabaseConnection,
    message_id: Uuid,
) -> Result<Vec<message_revision::Model>, AppError> {
    message_revision::Entity::find()
        .filter(message_revision::Column::MessageId.eq(message_id))
        .order_by_desc(message_revision::Column::Version)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 写入一条编辑历史版本。
async fn insert_revision<C: ConnectionTrait>(
    conn: &C,
    message_id: Uuid,
    version: i32,
    content: Value,
    editor_id: Option<Uuid>,
    created_at: DateTime<Utc>,
) -> Result<(), AppError> {
    message_revision::ActiveModel {
        id: Set(new_id()),
        message_id: Set(message_id),
        version: Set(version),
        content: Set(content),
        editor_id: Set(editor_id),
        created_at: Set(created_at.fixed_offset()),
    }
    .insert(conn)
    .await
    .map(|_| ())
    .map_err(map_db_err)
}

/// 编辑消息正文并在同一事务内追加版本历史。
///
/// 首次编辑时把原内容补录为 `version=1`（编辑者为发送者、时间为发送时间），
/// 新正文写入 `version=2`；此后每次编辑版本号递增。消息行的 `edited_at` 同步更新。
pub async fn edit_message_content(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    message_id: Uuid,
    content: Value,
    editor_id: Uuid,
    now: DateTime<Utc>,
) -> Result<message::Model, AppError> {
    let txn = db.begin().await.map_err(map_db_err)?;
    // 锁住消息行，串行化同一消息的并发编辑，避免版本号冲突
    let model = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::Id.eq(message_id))
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| AppError::not_found("IM_MESSAGE_NOT_FOUND", "消息不存在"))?;
    let latest = message_revision::Entity::find()
        .filter(message_revision::Column::MessageId.eq(message_id))
        .order_by_desc(message_revision::Column::Version)
        .one(&txn)
        .await
        .map_err(map_db_err)?;
    let next_version = match latest {
        Some(row) => row.version + 1,
        None => {
            insert_revision(
                &txn,
                message_id,
                1,
                model.content.clone(),
                model.sender_id,
                model.created_at.with_timezone(&Utc),
            )
            .await?;
            2
        }
    };
    insert_revision(
        &txn,
        message_id,
        next_version,
        content.clone(),
        Some(editor_id),
        now,
    )
    .await?;
    let mut active: message::ActiveModel = model.into();
    active.content = Set(content);
    active.edited_at = Set(Some(now.fixed_offset()));
    let updated = active.update(&txn).await.map_err(map_db_err)?;
    txn.commit().await.map_err(map_db_err)?;
    Ok(updated)
}

/// 列出某线程根下的全部回复（按 seq 升序）。
pub async fn list_thread_messages(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    root_id: Uuid,
) -> Result<Vec<message::Model>, AppError> {
    message::Entity::find()
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::ThreadRootId.eq(root_id))
        .order_by_asc(message::Column::Seq)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 统计一组根消息的线程回复数（根 ID → 条数；无回复的根不在结果中）。
pub async fn thread_reply_counts(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    root_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, i64>, AppError> {
    if root_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<Option<Uuid>> = message::Entity::find()
        .select_only()
        .column(message::Column::ThreadRootId)
        .filter(message::Column::ConversationId.eq(conversation_id))
        .filter(message::Column::ThreadRootId.is_in(root_ids.to_vec()))
        .into_tuple::<Option<Uuid>>()
        .all(db)
        .await
        .map_err(map_db_err)?;
    let mut counts: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    for root in rows.into_iter().flatten() {
        *counts.entry(root).or_insert(0) += 1;
    }
    Ok(counts)
}

/// 一条消息上某个表情的聚合。
#[derive(Debug, Clone)]
pub struct ReactionGroup {
    /// 表情。
    pub emoji: String,
    /// 回应人数。
    pub count: i64,
    /// 回应用户（用于判断 `mine` 与展示提示）。
    pub user_ids: Vec<Uuid>,
}

/// 切换表情回应：已存在则删除，返回是否新增。
pub async fn toggle_reaction(
    db: &DatabaseConnection,
    message_id: Uuid,
    user_id: Uuid,
    emoji: &str,
    now: DateTime<Utc>,
) -> Result<bool, AppError> {
    let existing = message_reaction::Entity::find_by_id((message_id, user_id, emoji.to_string()))
        .one(db)
        .await
        .map_err(map_db_err)?;
    if existing.is_some() {
        message_reaction::Entity::delete_by_id((message_id, user_id, emoji.to_string()))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        return Ok(false);
    }
    message_reaction::ActiveModel {
        message_id: Set(message_id),
        user_id: Set(user_id),
        emoji: Set(emoji.to_string()),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    Ok(true)
}

/// 批量读取消息的回应聚合（按表情首次出现时间排序）。
pub async fn reactions_for_messages(
    db: &DatabaseConnection,
    message_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<ReactionGroup>>, AppError> {
    let mut grouped: std::collections::HashMap<Uuid, Vec<ReactionGroup>> =
        std::collections::HashMap::new();
    if message_ids.is_empty() {
        return Ok(grouped);
    }
    let rows = message_reaction::Entity::find()
        .filter(message_reaction::Column::MessageId.is_in(message_ids.to_vec()))
        .order_by_asc(message_reaction::Column::CreatedAt)
        .all(db)
        .await
        .map_err(map_db_err)?;
    for row in rows {
        let groups = grouped.entry(row.message_id).or_default();
        match groups.iter_mut().find(|group| group.emoji == row.emoji) {
            Some(group) => {
                group.count += 1;
                group.user_ids.push(row.user_id);
            }
            None => groups.push(ReactionGroup {
                emoji: row.emoji,
                count: 1,
                user_ids: vec![row.user_id],
            }),
        }
    }
    Ok(grouped)
}

/// 在给定会话集合中搜索正文（大小写不敏感的子串匹配，按 seq 倒序）。
///
/// 关键词里的 LIKE 通配符会被转义（Postgres 默认转义符为反斜杠），按字面匹配。
pub async fn search_messages(
    db: &DatabaseConnection,
    conversation_ids: &[Uuid],
    query: &str,
    limit: u64,
) -> Result<Vec<message::Model>, AppError> {
    if conversation_ids.is_empty() {
        return Ok(Vec::new());
    }
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let placeholders = (1..=conversation_ids.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let query_index = conversation_ids.len() + 1;
    let limit_index = conversation_ids.len() + 2;
    let sql = format!(
        "SELECT id, conversation_id, seq, sender_id, type, content, reply_to_id, \
                client_msg_id, mentions, thread_root_id, edited_at, status, recalled_at, created_at \
         FROM messages \
         WHERE conversation_id IN ({placeholders}) \
           AND status <> 'recalled' \
           AND (content->>'text') ILIKE ${query_index} \
         ORDER BY seq DESC LIMIT ${limit_index}"
    );
    let mut values: Vec<SqlValue> = conversation_ids
        .iter()
        .map(|id| SqlValue::Uuid(Some(*id)))
        .collect();
    values.push(SqlValue::String(Some(format!("%{escaped}%"))));
    values.push(SqlValue::BigInt(Some(limit as i64)));
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(map_db_err)?;
    rows.iter()
        .map(|row| message::Model::from_query_result(row, "").map_err(map_db_err))
        .collect()
}

/// 设置成员的通知级别（all / mentions / mute）。
pub async fn set_member_notify_level(
    db: &DatabaseConnection,
    conversation_id: Uuid,
    user_id: Uuid,
    level: &str,
) -> Result<conversation_member::Model, AppError> {
    let member = find_member(db, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("IM_MEMBER_NOT_FOUND", "成员不存在"))?;
    let mut active: conversation_member::ActiveModel = member.into();
    active.notify_level = Set(level.to_string());
    active.update(db).await.map_err(map_db_err)
}
