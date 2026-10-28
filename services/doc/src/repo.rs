//! 数据访问层：只操作 doc schema。

use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QueryOrder, Set, Statement,
};
use uuid::Uuid;

use club_common::{new_id, AppError};

use crate::entity::{node, space, space_member, version};

/// 数据库错误 → 统一错误。
pub fn map_db_err(err: DbErr) -> AppError {
    AppError::internal(err)
}

/// 创建空间并授权所有者。
pub async fn create_space(
    db: &DatabaseConnection,
    owner_id: Uuid,
    name: &str,
    kind: &str,
    now: DateTime<Utc>,
) -> Result<space::Model, AppError> {
    let model = space::ActiveModel {
        id: Set(new_id()),
        name: Set(name.to_string()),
        r#type: Set(kind.to_string()),
        owner_id: Set(owner_id),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    space_member::ActiveModel {
        space_id: Set(model.id),
        user_id: Set(owner_id),
        role: Set(space_member::ROLE_ADMIN.to_string()),
        joined_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    Ok(model)
}

/// 用户可访问的空间。
pub async fn list_spaces(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<space::Model>, AppError> {
    let members = space_member::Entity::find()
        .filter(space_member::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let ids: Vec<Uuid> = members.iter().map(|m| m.space_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    space::Entity::find()
        .filter(space::Column::Id.is_in(ids))
        .order_by_asc(space::Column::CreatedAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 校验成员并返回成员记录。
pub async fn ensure_member(
    db: &DatabaseConnection,
    space_id: Uuid,
    user_id: Uuid,
) -> Result<space_member::Model, AppError> {
    space_member::Entity::find_by_id((space_id, user_id))
        .one(db)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| AppError::forbidden("DOC_NOT_MEMBER", "无权访问该空间"))
}

/// 删除空间：级联清理版本、节点与成员关系。
pub async fn delete_space(db: &DatabaseConnection, space_id: Uuid) -> Result<(), AppError> {
    let node_ids: Vec<Uuid> = node::Entity::find()
        .filter(node::Column::SpaceId.eq(space_id))
        .all(db)
        .await
        .map_err(map_db_err)?
        .into_iter()
        .map(|row| row.id)
        .collect();
    if !node_ids.is_empty() {
        version::Entity::delete_many()
            .filter(version::Column::NodeId.is_in(node_ids.clone()))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        node::Entity::delete_many()
            .filter(node::Column::Id.is_in(node_ids))
            .exec(db)
            .await
            .map_err(map_db_err)?;
    }
    space_member::Entity::delete_many()
        .filter(space_member::Column::SpaceId.eq(space_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    space::Entity::delete_by_id(space_id)
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(())
}

/// 校验可编辑（admin/editor）。
pub async fn ensure_editor(
    db: &DatabaseConnection,
    space_id: Uuid,
    user_id: Uuid,
) -> Result<space_member::Model, AppError> {
    let member = ensure_member(db, space_id, user_id).await?;
    if matches!(
        member.role.as_str(),
        space_member::ROLE_ADMIN | space_member::ROLE_EDITOR
    ) {
        Ok(member)
    } else {
        Err(AppError::forbidden(
            "DOC_FORBIDDEN",
            "仅管理员/编辑者可修改",
        ))
    }
}

/// 创建节点（文件夹/页面）。
pub async fn create_node(
    db: &DatabaseConnection,
    space_id: Uuid,
    parent_id: Option<Uuid>,
    kind: &str,
    title: &str,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<node::Model, AppError> {
    // 同级 position：现有最大 +1（单实例够用，冲突时排序稳定即可）
    let max_position: Option<i64> = node::Entity::find()
        .filter(node::Column::SpaceId.eq(space_id))
        .filter(match parent_id {
            Some(parent) => node::Column::ParentId.eq(parent),
            None => node::Column::ParentId.is_null(),
        })
        .order_by_desc(node::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    node::ActiveModel {
        id: Set(new_id()),
        space_id: Set(space_id),
        parent_id: Set(parent_id),
        kind: Set(kind.to_string()),
        title: Set(title.to_string()),
        position: Set(max_position.unwrap_or(0) + 1),
        version: Set(1),
        content_md: Set(String::new()),
        updated_by: Set(user_id),
        visibility: Set("space".to_string()),
        share_token: Set(None),
        share_password_hash: Set(None),
        share_expires_at: Set(None),
        share_permission: Set(None),
        deleted_at: Set(None),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 查找节点（含已删除）。
pub async fn find_node(
    db: &DatabaseConnection,
    node_id: Uuid,
) -> Result<Option<node::Model>, AppError> {
    node::Entity::find_by_id(node_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 空间全部有效节点（树构建用）。
pub async fn list_nodes(
    db: &DatabaseConnection,
    space_id: Uuid,
) -> Result<Vec<node::Model>, AppError> {
    node::Entity::find()
        .filter(node::Column::SpaceId.eq(space_id))
        .filter(node::Column::DeletedAt.is_null())
        .order_by_asc(node::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 乐观锁更新正文：`base_version` 不匹配返回 409；成功则版本 +1 并留快照。
pub async fn update_content(
    db: &DatabaseConnection,
    node: &node::Model,
    base_version: i64,
    content: &str,
    editor: Uuid,
    now: DateTime<Utc>,
) -> Result<node::Model, AppError> {
    if node.version != base_version {
        return Err(AppError::conflict(
            "DOC_VERSION_CONFLICT",
            "内容已被他人修改，请刷新后重试",
        ));
    }
    // 快照当前版本
    version::ActiveModel {
        id: Set(new_id()),
        node_id: Set(node.id),
        version: Set(node.version),
        content_md: Set(node.content_md.clone()),
        author_id: Set(node.updated_by),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;

    // 条件更新：version = base 才生效（并发下仅一个成功）
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE nodes SET content_md = $2, version = version + 1, updated_by = $3, updated_at = $4 \
         WHERE id = $1 AND version = $5 RETURNING version",
        vec![
            node.id.into(),
            content.into(),
            editor.into(),
            now.fixed_offset().into(),
            base_version.into(),
        ],
    );
    let row = db.query_one_raw(statement).await.map_err(map_db_err)?;
    if row.is_none() {
        return Err(AppError::conflict(
            "DOC_VERSION_CONFLICT",
            "内容已被他人修改，请刷新后重试",
        ));
    }
    find_node(db, node.id)
        .await?
        .ok_or_else(|| AppError::not_found("DOC_NODE_NOT_FOUND", "节点不存在"))
}

/// 历史版本列表（新到旧）。
pub async fn list_versions(
    db: &DatabaseConnection,
    node_id: Uuid,
) -> Result<Vec<version::Model>, AppError> {
    version::Entity::find()
        .filter(version::Column::NodeId.eq(node_id))
        .order_by_desc(version::Column::Version)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 查找指定历史版本。
pub async fn find_version(
    db: &DatabaseConnection,
    node_id: Uuid,
    version_number: i64,
) -> Result<Option<version::Model>, AppError> {
    version::Entity::find()
        .filter(version::Column::NodeId.eq(node_id))
        .filter(version::Column::Version.eq(version_number))
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 当前位置（数字越小越靠前）。
pub fn next_position_from(max: Option<i64>) -> i64 {
    max.unwrap_or(0) + 1
}

/// 计算某节点的全部子孙 ID（仅未删除节点，含自身）。
pub async fn descendant_ids(
    db: &DatabaseConnection,
    space_id: Uuid,
    node_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let nodes = list_nodes(db, space_id).await?;
    let mut children: std::collections::HashMap<Option<Uuid>, Vec<Uuid>> =
        std::collections::HashMap::new();
    for node in &nodes {
        children.entry(node.parent_id).or_default().push(node.id);
    }
    let mut result = vec![node_id];
    let mut index = 0;
    while index < result.len() {
        let current = result[index];
        index += 1;
        if let Some(kids) = children.get(&Some(current)) {
            result.extend(kids.iter().copied());
        }
    }
    Ok(result)
}

/// 节点元信息变更（未提供的字段保持不变）。
#[derive(Debug, Default)]
pub struct NodeMetaUpdate<'a> {
    /// 新标题。
    pub title: Option<&'a str>,
    /// 新父节点（`Some(None)` 表示移到根）。
    pub parent_id: Option<Option<Uuid>>,
    /// 同级排序。
    pub position: Option<i64>,
    /// 可见性。
    pub visibility: Option<&'a str>,
}

/// 更新节点元信息（标题/父节点/排序/可见性）。
pub async fn update_node_meta(
    db: &DatabaseConnection,
    node: &node::Model,
    change: NodeMetaUpdate<'_>,
    editor: Uuid,
    now: DateTime<Utc>,
) -> Result<node::Model, AppError> {
    let mut active: node::ActiveModel = node.clone().into();
    if let Some(visibility) = change.visibility {
        active.visibility = Set(visibility.to_string());
    }
    if let Some(title) = change.title {
        active.title = Set(title.to_string());
    }
    if let Some(parent) = change.parent_id {
        // 移动到新父级且未显式指定排序时，排到同级末尾
        if change.position.is_none() && parent != node.parent_id {
            let max_position: Option<i64> = node::Entity::find()
                .filter(node::Column::SpaceId.eq(node.space_id))
                .filter(node::Column::DeletedAt.is_null())
                .filter(match parent {
                    Some(parent) => node::Column::ParentId.eq(parent),
                    None => node::Column::ParentId.is_null(),
                })
                .order_by_desc(node::Column::Position)
                .one(db)
                .await
                .map_err(map_db_err)?
                .map(|row| row.position);
            active.position = Set(max_position.unwrap_or(0) + 1);
        }
        active.parent_id = Set(parent);
    }
    if let Some(position) = change.position {
        active.position = Set(position);
    }
    active.updated_by = Set(editor);
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 软删节点及其全部子孙。
pub async fn soft_delete_subtree(
    db: &DatabaseConnection,
    space_id: Uuid,
    node_id: Uuid,
    now: DateTime<Utc>,
) -> Result<u64, AppError> {
    let ids = descendant_ids(db, space_id, node_id).await?;
    let result = node::Entity::update_many()
        .col_expr(node::Column::DeletedAt, Expr::value(now.fixed_offset()))
        .filter(node::Column::Id.is_in(ids))
        .filter(node::Column::DeletedAt.is_null())
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(result.rows_affected)
}

/// 恢复节点及其全部子孙（含已删除的子孙）。
pub async fn restore_subtree(
    db: &DatabaseConnection,
    space_id: Uuid,
    node_id: Uuid,
    now: DateTime<Utc>,
) -> Result<u64, AppError> {
    // 含已删除节点，才能把整棵子树捞回来
    let nodes = node::Entity::find()
        .filter(node::Column::SpaceId.eq(space_id))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let mut children: std::collections::HashMap<Option<Uuid>, Vec<Uuid>> =
        std::collections::HashMap::new();
    for node in &nodes {
        children.entry(node.parent_id).or_default().push(node.id);
    }
    let mut ids = vec![node_id];
    let mut index = 0;
    while index < ids.len() {
        let current = ids[index];
        index += 1;
        if let Some(kids) = children.get(&Some(current)) {
            ids.extend(kids.iter().copied());
        }
    }
    let result = node::Entity::update_many()
        .col_expr(
            node::Column::DeletedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .filter(node::Column::Id.is_in(ids))
        .filter(node::Column::DeletedAt.is_not_null())
        .exec(db)
        .await
        .map_err(map_db_err)?;
    let _ = now;
    Ok(result.rows_affected)
}

/// 按分享 token 查节点（含已删除，由调用方判断）。
pub async fn find_node_by_share_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<node::Model>, AppError> {
    node::Entity::find()
        .filter(node::Column::ShareToken.eq(token))
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 写入/更新分享设置。
pub async fn set_share(
    db: &DatabaseConnection,
    node: &node::Model,
    token: &str,
    password_hash: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
    permission: &str,
    now: DateTime<Utc>,
) -> Result<node::Model, AppError> {
    let mut active: node::ActiveModel = node.clone().into();
    active.share_token = Set(Some(token.to_string()));
    active.share_password_hash = Set(password_hash.map(str::to_string));
    active.share_expires_at = Set(expires_at.map(|value| value.fixed_offset()));
    active.share_permission = Set(Some(permission.to_string()));
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 关闭分享（清空 token 与密码）。
pub async fn clear_share(
    db: &DatabaseConnection,
    node: &node::Model,
    now: DateTime<Utc>,
) -> Result<node::Model, AppError> {
    let mut active: node::ActiveModel = node.clone().into();
    active.share_token = Set(None);
    active.share_password_hash = Set(None);
    active.share_expires_at = Set(None);
    active.share_permission = Set(None);
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 搜索结果行。
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// 节点 ID。
    pub id: Uuid,
    /// 空间 ID。
    pub space_id: Uuid,
    /// 空间名称。
    pub space_name: String,
    /// 标题。
    pub title: String,
    /// 命中上下文摘要。
    pub snippet: String,
    /// 更新时间。
    pub updated_at: DateTime<Utc>,
}

/// 生成本文摘要（命中位置前后各 40 字，压平换行）。
fn make_snippet(content: &str, query: &str) -> String {
    let flat = content.replace('\n', " ");
    let lower = flat.to_lowercase();
    let needle = query.to_lowercase();
    let Some(byte_index) = lower.find(&needle) else {
        return flat.chars().take(80).collect();
    };
    let char_index = flat[..byte_index].chars().count();
    let start = char_index.saturating_sub(40);
    let total = flat.chars().count();
    let end = (char_index + needle.chars().count() + 40).min(total);
    let snippet: String = flat.chars().skip(start).take(end - start).collect();
    if start > 0 {
        format!("…{snippet}")
    } else if end < total {
        format!("{snippet}…")
    } else {
        snippet
    }
}

/// 在我可读的空间里搜索标题与正文。
pub async fn search_docs(
    db: &DatabaseConnection,
    user_id: Uuid,
    query: &str,
    limit: u64,
) -> Result<Vec<SearchHit>, AppError> {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT n.id, n.space_id, s.name, n.title, n.content_md, n.updated_at \
             FROM nodes n \
             JOIN space_members m ON m.space_id = n.space_id AND m.user_id = $1 \
             JOIN spaces s ON s.id = n.space_id \
             WHERE n.deleted_at IS NULL AND n.kind = 'page' \
               AND (m.role IN ('admin', 'editor') OR n.visibility <> 'private') \
               AND (n.title ILIKE $2 OR n.content_md ILIKE $2) \
             ORDER BY n.updated_at DESC \
             LIMIT $3",
            vec![
                sea_orm::sea_query::Value::Uuid(Some(user_id)),
                sea_orm::sea_query::Value::String(Some(pattern)),
                sea_orm::sea_query::Value::BigInt(Some(limit as i64)),
            ],
        ))
        .await
        .map_err(map_db_err)?;
    let mut hits = Vec::with_capacity(rows.len());
    for row in rows {
        let content: String = row.try_get_by_index(4).map_err(map_db_err)?;
        hits.push(SearchHit {
            id: row.try_get_by_index(0).map_err(map_db_err)?,
            space_id: row.try_get_by_index(1).map_err(map_db_err)?,
            space_name: row.try_get_by_index(2).map_err(map_db_err)?,
            title: row.try_get_by_index(3).map_err(map_db_err)?,
            snippet: make_snippet(&content, query),
            updated_at: row.try_get_by_index(5).map_err(map_db_err)?,
        });
    }
    Ok(hits)
}

/// 写入文档媒体文件。
pub async fn insert_file(
    db: &DatabaseConnection,
    uploader_id: Uuid,
    name: &str,
    mime: &str,
    data: Vec<u8>,
    now: DateTime<Utc>,
) -> Result<crate::entity::doc_file::Model, AppError> {
    use crate::entity::doc_file;
    doc_file::ActiveModel {
        id: Set(new_id()),
        uploader_id: Set(uploader_id),
        name: Set(name.to_string()),
        mime: Set(mime.to_string()),
        size: Set(data.len() as i32),
        data: Set(data),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 按 ID 读取文档媒体文件。
pub async fn find_file(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<Option<crate::entity::doc_file::Model>, AppError> {
    crate::entity::doc_file::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(map_db_err)
}
