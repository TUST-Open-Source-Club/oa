//! 数据访问层：只操作 task schema。

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use club_common::{new_id, AppError};

use crate::entity::{
    board, column, comment, label, project, project_member, subtask, task, task_assignee,
    task_label,
};

/// 数据库错误 → 统一错误。
pub fn map_db_err(err: DbErr) -> AppError {
    AppError::internal(err)
}

/// 创建项目（owner 为管理员）。
pub async fn create_project(
    db: &DatabaseConnection,
    owner_id: Uuid,
    name: &str,
    description: &str,
    now: DateTime<Utc>,
) -> Result<project::Model, AppError> {
    let model = project::ActiveModel {
        id: Set(new_id()),
        name: Set(name.to_string()),
        description: Set(description.to_string()),
        owner_id: Set(owner_id),
        archived_at: Set(None),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    project_member::ActiveModel {
        project_id: Set(model.id),
        user_id: Set(owner_id),
        role: Set(project_member::ROLE_ADMIN.to_string()),
        joined_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    // 默认看板
    let board = board::ActiveModel {
        id: Set(new_id()),
        project_id: Set(model.id),
        name: Set("默认看板".to_string()),
        position: Set(0),
        archived_at: Set(None),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;
    // 默认三列：待办 / 进行中 / 已完成
    for (name, position, is_done) in [
        ("待办", 1, false),
        ("进行中", 2, false),
        ("已完成", 3, true),
    ] {
        column::ActiveModel {
            id: Set(new_id()),
            project_id: Set(model.id),
            board_id: Set(board.id),
            name: Set(name.to_string()),
            position: Set(position),
            is_done: Set(is_done),
            created_at: Set(now.fixed_offset()),
        }
        .insert(db)
        .await
        .map_err(map_db_err)?;
    }
    Ok(model)
}

/// 用户参与的项目（不含归档）。
pub async fn list_projects(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<project::Model>, AppError> {
    let members = project_member::Entity::find()
        .filter(project_member::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let ids: Vec<Uuid> = members.iter().map(|m| m.project_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    project::Entity::find()
        .filter(project::Column::Id.is_in(ids))
        .filter(project::Column::ArchivedAt.is_null())
        .order_by_desc(project::Column::CreatedAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 校验项目成员。
pub async fn ensure_member(
    db: &DatabaseConnection,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<project_member::Model, AppError> {
    project_member::Entity::find_by_id((project_id, user_id))
        .one(db)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| AppError::forbidden("TASK_NOT_MEMBER", "无权访问该项目"))
}

/// 校验项目管理员。
pub async fn ensure_admin(
    db: &DatabaseConnection,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<project_member::Model, AppError> {
    let member = ensure_member(db, project_id, user_id).await?;
    if member.role != project_member::ROLE_ADMIN {
        return Err(AppError::forbidden("TASK_FORBIDDEN", "仅项目管理员可操作"));
    }
    Ok(member)
}

/// 添加项目成员（幂等）。
pub async fn add_member(
    db: &DatabaseConnection,
    project_id: Uuid,
    user_id: Uuid,
    role: &str,
    now: DateTime<Utc>,
) -> Result<project_member::Model, AppError> {
    if let Some(existing) = project_member::Entity::find_by_id((project_id, user_id))
        .one(db)
        .await
        .map_err(map_db_err)?
    {
        return Ok(existing);
    }
    project_member::ActiveModel {
        project_id: Set(project_id),
        user_id: Set(user_id),
        role: Set(role.to_string()),
        joined_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 新建看板列。
pub async fn create_column(
    db: &DatabaseConnection,
    project_id: Uuid,
    board_id: Uuid,
    name: &str,
    now: DateTime<Utc>,
) -> Result<column::Model, AppError> {
    let max: Option<i64> = column::Entity::find()
        .filter(column::Column::BoardId.eq(board_id))
        .order_by_desc(column::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    column::ActiveModel {
        id: Set(new_id()),
        project_id: Set(project_id),
        board_id: Set(board_id),
        name: Set(name.to_string()),
        position: Set(max.unwrap_or(0) + 1),
        is_done: Set(false),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 列出看板列。
pub async fn list_columns(
    db: &DatabaseConnection,
    project_id: Uuid,
) -> Result<Vec<column::Model>, AppError> {
    // 兼容旧接口：返回默认看板（position 最小）的列
    let board = default_board(db, project_id).await?;
    match board {
        Some(board) => list_columns_by_board(db, board.id).await,
        None => Ok(Vec::new()),
    }
}

/// 看板下的列。
pub async fn list_columns_by_board(
    db: &DatabaseConnection,
    board_id: Uuid,
) -> Result<Vec<column::Model>, AppError> {
    column::Entity::find()
        .filter(column::Column::BoardId.eq(board_id))
        .order_by_asc(column::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 查找列。
pub async fn find_column(
    db: &DatabaseConnection,
    column_id: Uuid,
) -> Result<Option<column::Model>, AppError> {
    column::Entity::find_by_id(column_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 创建任务。
#[allow(clippy::too_many_arguments)]
pub async fn create_task(
    db: &DatabaseConnection,
    project_id: Uuid,
    column_id: Uuid,
    title: &str,
    description_md: &str,
    assignee_id: Option<Uuid>,
    priority: &str,
    due_at: Option<DateTime<Utc>>,
    start_at: Option<DateTime<Utc>>,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<task::Model, AppError> {
    let max: Option<i64> = task::Entity::find()
        .filter(task::Column::ColumnId.eq(column_id))
        .order_by_desc(task::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    task::ActiveModel {
        id: Set(new_id()),
        project_id: Set(project_id),
        column_id: Set(column_id),
        title: Set(title.to_string()),
        description_md: Set(description_md.to_string()),
        assignee_id: Set(assignee_id),
        start_at: Set(start_at.map(|value| value.fixed_offset())),
        priority: Set(priority.to_string()),
        due_at: Set(due_at.map(|value| value.fixed_offset())),
        position: Set(max.unwrap_or(0) + 1),
        created_by: Set(user_id),
        status: Set("active".to_string()),
        completed_at: Set(None),
        cover_attachment_id: Set(None),
        archived_at: Set(None),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 更新项目名称/描述（None 表示不修改）。
pub async fn update_project(
    db: &DatabaseConnection,
    project: &project::Model,
    name: Option<String>,
    description: Option<String>,
) -> Result<project::Model, AppError> {
    let mut active: project::ActiveModel = project.clone().into();
    if let Some(name) = name {
        active.name = Set(name);
    }
    if let Some(description) = description {
        active.description = Set(description);
    }
    active.update(db).await.map_err(map_db_err)
}

/// 删除项目：级联任务（负责人/附件/子任务/评论）、看板列与成员。
pub async fn delete_project(db: &DatabaseConnection, project_id: Uuid) -> Result<(), AppError> {
    use crate::entity::{comment, subtask, task_assignee, task_attachment};
    let task_ids: Vec<Uuid> = task::Entity::find()
        .filter(task::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_db_err)?
        .into_iter()
        .map(|row| row.id)
        .collect();
    if !task_ids.is_empty() {
        task_assignee::Entity::delete_many()
            .filter(task_assignee::Column::TaskId.is_in(task_ids.clone()))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        task_attachment::Entity::delete_many()
            .filter(task_attachment::Column::TaskId.is_in(task_ids.clone()))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        subtask::Entity::delete_many()
            .filter(subtask::Column::TaskId.is_in(task_ids.clone()))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        comment::Entity::delete_many()
            .filter(comment::Column::TaskId.is_in(task_ids))
            .exec(db)
            .await
            .map_err(map_db_err)?;
        task::Entity::delete_many()
            .filter(task::Column::ProjectId.eq(project_id))
            .exec(db)
            .await
            .map_err(map_db_err)?;
    }
    column::Entity::delete_many()
        .filter(column::Column::ProjectId.eq(project_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    project_member::Entity::delete_many()
        .filter(project_member::Column::ProjectId.eq(project_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    project::Entity::delete_by_id(project_id)
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(())
}

/// 按 ID 查询项目。
pub async fn find_project(
    db: &DatabaseConnection,
    project_id: Uuid,
) -> Result<Option<project::Model>, AppError> {
    project::Entity::find_by_id(project_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 查找任务。
pub async fn find_task(
    db: &DatabaseConnection,
    task_id: Uuid,
) -> Result<Option<task::Model>, AppError> {
    task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 任务列表（可按列/负责人过滤）。
pub async fn list_tasks(
    db: &DatabaseConnection,
    project_id: Uuid,
    column_id: Option<Uuid>,
    assignee_id: Option<Uuid>,
) -> Result<Vec<task::Model>, AppError> {
    let mut select = task::Entity::find().filter(task::Column::ProjectId.eq(project_id));
    if let Some(column_id) = column_id {
        select = select.filter(task::Column::ColumnId.eq(column_id));
    }
    if let Some(assignee_id) = assignee_id {
        select = select.filter(task::Column::AssigneeId.eq(assignee_id));
    }
    select
        .order_by_asc(task::Column::ColumnId)
        .order_by_asc(task::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 更新任务字段（None 表示不修改）。
#[allow(clippy::too_many_arguments)]
pub async fn update_task(
    db: &DatabaseConnection,
    task: &task::Model,
    title: Option<String>,
    description_md: Option<String>,
    assignee_id: Option<Option<Uuid>>,
    priority: Option<String>,
    due_at: Option<Option<DateTime<Utc>>>,
    start_at: Option<Option<DateTime<Utc>>>,
    now: DateTime<Utc>,
) -> Result<task::Model, AppError> {
    let mut active: task::ActiveModel = task.clone().into();
    if let Some(title) = title {
        active.title = Set(title);
    }
    if let Some(description_md) = description_md {
        active.description_md = Set(description_md);
    }
    if let Some(assignee_id) = assignee_id {
        active.assignee_id = Set(assignee_id);
    }
    if let Some(priority) = priority {
        active.priority = Set(priority);
    }
    if let Some(due_at) = due_at {
        active.due_at = Set(due_at.map(|value| value.fixed_offset()));
    }
    if let Some(start_at) = start_at {
        active.start_at = Set(start_at.map(|value| value.fixed_offset()));
    }
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 移动任务到目标列/位置（进入完成列时记录完成时间）。
pub async fn move_task(
    db: &DatabaseConnection,
    task: &task::Model,
    target: &column::Model,
    now: DateTime<Utc>,
) -> Result<task::Model, AppError> {
    let max: Option<i64> = task::Entity::find()
        .filter(task::Column::ColumnId.eq(target.id))
        .order_by_desc(task::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    let mut active: task::ActiveModel = task.clone().into();
    active.column_id = Set(target.id);
    active.position = Set(max.unwrap_or(0) + 1);
    active.completed_at = Set(if target.is_done {
        Some(now.fixed_offset())
    } else {
        None
    });
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 我的任务（跨项目）。
pub async fn my_tasks(
    db: &DatabaseConnection,
    user_id: Uuid,
    due_before: Option<DateTime<Utc>>,
    only_open: bool,
) -> Result<Vec<task::Model>, AppError> {
    let members = project_member::Entity::find()
        .filter(project_member::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let ids: Vec<Uuid> = members.iter().map(|m| m.project_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut select = task::Entity::find()
        .filter(task::Column::ProjectId.is_in(ids))
        .filter(task::Column::AssigneeId.eq(user_id));
    if let Some(due_before) = due_before {
        // NULL 截止时间自然不满足 lte
        select = select.filter(task::Column::DueAt.lte(due_before.fixed_offset()));
    }
    if only_open {
        select = select.filter(task::Column::CompletedAt.is_null());
    }
    select
        .order_by_asc(task::Column::DueAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 创建子任务。
pub async fn create_subtask(
    db: &DatabaseConnection,
    task_id: Uuid,
    title: &str,
    now: DateTime<Utc>,
) -> Result<subtask::Model, AppError> {
    let max: Option<i64> = subtask::Entity::find()
        .filter(subtask::Column::TaskId.eq(task_id))
        .order_by_desc(subtask::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    subtask::ActiveModel {
        id: Set(new_id()),
        task_id: Set(task_id),
        title: Set(title.to_string()),
        done: Set(false),
        position: Set(max.unwrap_or(0) + 1),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 列出子任务。
pub async fn list_subtasks(
    db: &DatabaseConnection,
    task_id: Uuid,
) -> Result<Vec<subtask::Model>, AppError> {
    subtask::Entity::find()
        .filter(subtask::Column::TaskId.eq(task_id))
        .order_by_asc(subtask::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 切换子任务完成状态。
pub async fn set_subtask_done(
    db: &DatabaseConnection,
    subtask: &subtask::Model,
    done: bool,
) -> Result<subtask::Model, AppError> {
    let mut active: subtask::ActiveModel = subtask.clone().into();
    active.done = Set(done);
    active.update(db).await.map_err(map_db_err)
}

/// 创建评论。
pub async fn create_comment(
    db: &DatabaseConnection,
    task_id: Uuid,
    author_id: Uuid,
    content_md: &str,
    now: DateTime<Utc>,
) -> Result<comment::Model, AppError> {
    comment::ActiveModel {
        id: Set(new_id()),
        task_id: Set(task_id),
        author_id: Set(author_id),
        content_md: Set(content_md.to_string()),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 列出评论。
pub async fn list_comments(
    db: &DatabaseConnection,
    task_id: Uuid,
) -> Result<Vec<comment::Model>, AppError> {
    comment::Entity::find()
        .filter(comment::Column::TaskId.eq(task_id))
        .order_by_asc(comment::Column::CreatedAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 覆盖任务负责人（先清空再写入）。
pub async fn set_task_assignees(
    db: &DatabaseConnection,
    task_id: Uuid,
    user_ids: &[Uuid],
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    use crate::entity::task_assignee;
    task_assignee::Entity::delete_many()
        .filter(task_assignee::Column::TaskId.eq(task_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    for user_id in user_ids {
        task_assignee::ActiveModel {
            task_id: Set(task_id),
            user_id: Set(*user_id),
            assigned_at: Set(now.fixed_offset()),
        }
        .insert(db)
        .await
        .map_err(map_db_err)?;
    }
    Ok(())
}

/// 任务负责人列表。
pub async fn list_task_assignees(
    db: &DatabaseConnection,
    task_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    use crate::entity::task_assignee;
    let rows = task_assignee::Entity::find()
        .filter(task_assignee::Column::TaskId.eq(task_id))
        .all(db)
        .await
        .map_err(map_db_err)?;
    Ok(rows.into_iter().map(|row| row.user_id).collect())
}

/// 更新任务状态（done 记录完成时间，active 清除）。
pub async fn update_task_status(
    db: &DatabaseConnection,
    task: &task::Model,
    status: &str,
    now: DateTime<Utc>,
) -> Result<task::Model, AppError> {
    let mut active: task::ActiveModel = task.clone().into();
    active.status = Set(status.to_string());
    active.completed_at = Set(if status == "done" {
        Some(now.fixed_offset())
    } else {
        None
    });
    active.updated_at = Set(now.fixed_offset());
    active.update(db).await.map_err(map_db_err)
}

/// 删除任务及其关联数据。
pub async fn delete_task(db: &DatabaseConnection, task_id: Uuid) -> Result<(), AppError> {
    use crate::entity::{comment, subtask, task_assignee};
    task_assignee::Entity::delete_many()
        .filter(task_assignee::Column::TaskId.eq(task_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    subtask::Entity::delete_many()
        .filter(subtask::Column::TaskId.eq(task_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    comment::Entity::delete_many()
        .filter(comment::Column::TaskId.eq(task_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    task::Entity::delete_by_id(task_id)
        .exec(db)
        .await
        .map_err(map_db_err)?;
    Ok(())
}

/// 按快照重建任务（撤销删除；负责人由调用方回填）。
pub async fn restore_task(
    db: &DatabaseConnection,
    snapshot: &serde_json::Value,
    fallback_created_by: Uuid,
    now: DateTime<Utc>,
) -> Result<task::Model, AppError> {
    fn uuid_of(value: Option<&serde_json::Value>) -> Option<Uuid> {
        value
            .and_then(serde_json::Value::as_str)
            .and_then(|text| text.parse().ok())
    }
    fn time_of(value: Option<&serde_json::Value>) -> Option<chrono::DateTime<chrono::FixedOffset>> {
        value
            .and_then(serde_json::Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
    }
    let missing = |field: &str| {
        AppError::unprocessable("TASK_UNDO_UNSUPPORTED", format!("快照缺少 {field}"), vec![])
    };
    let model = task::ActiveModel {
        id: Set(uuid_of(snapshot.get("id")).ok_or_else(|| missing("id"))?),
        project_id: Set(uuid_of(snapshot.get("projectId")).ok_or_else(|| missing("projectId"))?),
        column_id: Set(uuid_of(snapshot.get("columnId")).ok_or_else(|| missing("columnId"))?),
        title: Set(snapshot
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("未命名任务")
            .to_string()),
        description_md: Set(snapshot
            .get("descriptionMd")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()),
        assignee_id: Set(uuid_of(snapshot.get("assigneeId"))),
        start_at: Set(time_of(snapshot.get("startAt"))),
        priority: Set(snapshot
            .get("priority")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("normal")
            .to_string()),
        due_at: Set(time_of(snapshot.get("dueAt"))),
        position: Set(snapshot
            .get("position")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)),
        created_by: Set(fallback_created_by),
        archived_at: Set(None),
        cover_attachment_id: Set(None),
        status: Set(snapshot
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string()),
        completed_at: Set(time_of(snapshot.get("completedAt"))),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
    };
    model.insert(db).await.map_err(map_db_err)
}

/// 看板列表（不含归档）。
pub async fn list_boards(
    db: &DatabaseConnection,
    project_id: Uuid,
) -> Result<Vec<board::Model>, AppError> {
    board::Entity::find()
        .filter(board::Column::ProjectId.eq(project_id))
        .filter(board::Column::ArchivedAt.is_null())
        .order_by_asc(board::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 项目默认看板（position 最小）。
pub async fn default_board(
    db: &DatabaseConnection,
    project_id: Uuid,
) -> Result<Option<board::Model>, AppError> {
    board::Entity::find()
        .filter(board::Column::ProjectId.eq(project_id))
        .filter(board::Column::ArchivedAt.is_null())
        .order_by_asc(board::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 新建看板。
pub async fn create_board(
    db: &DatabaseConnection,
    project_id: Uuid,
    name: &str,
    now: DateTime<Utc>,
) -> Result<board::Model, AppError> {
    let max: Option<i64> = board::Entity::find()
        .filter(board::Column::ProjectId.eq(project_id))
        .order_by_desc(board::Column::Position)
        .one(db)
        .await
        .map_err(map_db_err)?
        .map(|row| row.position);
    board::ActiveModel {
        id: Set(new_id()),
        project_id: Set(project_id),
        name: Set(name.to_string()),
        position: Set(max.unwrap_or(0) + 1),
        archived_at: Set(None),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 查找看板。
pub async fn find_board(
    db: &DatabaseConnection,
    board_id: Uuid,
) -> Result<Option<board::Model>, AppError> {
    board::Entity::find_by_id(board_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 重命名看板。
pub async fn rename_board(
    db: &DatabaseConnection,
    board: &board::Model,
    name: &str,
) -> Result<board::Model, AppError> {
    let mut active: board::ActiveModel = board.clone().into();
    active.name = Set(name.to_string());
    active.update(db).await.map_err(map_db_err)
}

/// 删除看板（仅空看板）。
pub async fn delete_board(db: &DatabaseConnection, board_id: Uuid) -> Result<(), AppError> {
    board::Entity::delete_by_id(board_id)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(map_db_err)
}

/// 看板是否还有列。
pub async fn board_column_count(db: &DatabaseConnection, board_id: Uuid) -> Result<u64, AppError> {
    column::Entity::find()
        .filter(column::Column::BoardId.eq(board_id))
        .count(db)
        .await
        .map_err(map_db_err)
}

/// 看板任务筛选条件。
#[derive(Debug, Default, Clone)]
pub struct TaskFilter {
    /// 负责人。
    pub assignee_id: Option<Uuid>,
    /// 标签。
    pub label_id: Option<Uuid>,
    /// 标题关键词。
    pub keyword: Option<String>,
    /// 截止：overdue / today / week。
    pub due: Option<String>,
}

/// 看板下的任务（排除归档，可按负责人/标签/关键词/截止过滤）。
pub async fn list_tasks_by_board(
    db: &DatabaseConnection,
    board_id: Uuid,
    filter: &TaskFilter,
    now: DateTime<Utc>,
) -> Result<Vec<task::Model>, AppError> {
    let columns = list_columns_by_board(db, board_id).await?;
    let ids: Vec<Uuid> = columns.iter().map(|column| column.id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut select = task::Entity::find()
        .filter(task::Column::ColumnId.is_in(ids))
        .filter(task::Column::Status.ne("terminated"))
        .filter(task::Column::ArchivedAt.is_null());
    if let Some(assignee) = filter.assignee_id {
        let assigned: Vec<Uuid> = task_assignee::Entity::find()
            .filter(task_assignee::Column::UserId.eq(assignee))
            .all(db)
            .await
            .map_err(map_db_err)?
            .into_iter()
            .map(|row| row.task_id)
            .collect();
        select = select.filter(task::Column::Id.is_in(assigned));
    }
    if let Some(label_id) = filter.label_id {
        let tagged: Vec<Uuid> = task_label::Entity::find()
            .filter(task_label::Column::LabelId.eq(label_id))
            .all(db)
            .await
            .map_err(map_db_err)?
            .into_iter()
            .map(|row| row.task_id)
            .collect();
        select = select.filter(task::Column::Id.is_in(tagged));
    }
    if let Some(keyword) = filter
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        select = select.filter(task::Column::Title.contains(keyword));
    }
    match filter.due.as_deref() {
        Some("overdue") => {
            select = select.filter(task::Column::DueAt.lt(now.fixed_offset()));
        }
        Some("today") => {
            select = select
                .filter(task::Column::DueAt.gte(now.fixed_offset()))
                .filter(task::Column::DueAt.lt((now + chrono::Duration::days(1)).fixed_offset()));
        }
        Some("week") => {
            select = select
                .filter(task::Column::DueAt.gte(now.fixed_offset()))
                .filter(task::Column::DueAt.lt((now + chrono::Duration::days(7)).fixed_offset()));
        }
        _ => {}
    }
    select
        .order_by_asc(task::Column::Position)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 看板归档任务。
pub async fn list_archived_tasks(
    db: &DatabaseConnection,
    board_id: Uuid,
) -> Result<Vec<task::Model>, AppError> {
    let columns = list_columns_by_board(db, board_id).await?;
    let ids: Vec<Uuid> = columns.iter().map(|column| column.id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    task::Entity::find()
        .filter(task::Column::ColumnId.is_in(ids))
        .filter(task::Column::ArchivedAt.is_not_null())
        .order_by_desc(task::Column::ArchivedAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 归档卡片。
pub async fn archive_task(
    db: &DatabaseConnection,
    model: &task::Model,
    now: DateTime<Utc>,
    archived: bool,
) -> Result<task::Model, AppError> {
    let mut active: task::ActiveModel = model.clone().into();
    active.archived_at = Set(if archived {
        Some(now.fixed_offset())
    } else {
        None
    });
    active.update(db).await.map_err(map_db_err)
}

/// 删除空列（有任务时由调用方拦截）。
pub async fn delete_column(db: &DatabaseConnection, column_id: Uuid) -> Result<(), AppError> {
    column::Entity::delete_by_id(column_id)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(map_db_err)
}

/// 列内任务数。
pub async fn column_task_count(db: &DatabaseConnection, column_id: Uuid) -> Result<u64, AppError> {
    task::Entity::find()
        .filter(task::Column::ColumnId.eq(column_id))
        .count(db)
        .await
        .map_err(map_db_err)
}

/// 看板标签。
pub async fn list_labels(
    db: &DatabaseConnection,
    board_id: Uuid,
) -> Result<Vec<label::Model>, AppError> {
    label::Entity::find()
        .filter(label::Column::BoardId.eq(board_id))
        .order_by_asc(label::Column::CreatedAt)
        .all(db)
        .await
        .map_err(map_db_err)
}

/// 新建标签。
pub async fn create_label(
    db: &DatabaseConnection,
    board_id: Uuid,
    name: &str,
    color: &str,
    now: DateTime<Utc>,
) -> Result<label::Model, AppError> {
    label::ActiveModel {
        id: Set(new_id()),
        board_id: Set(board_id),
        name: Set(name.to_string()),
        color: Set(color.to_string()),
        created_at: Set(now.fixed_offset()),
    }
    .insert(db)
    .await
    .map_err(map_db_err)
}

/// 查找标签。
pub async fn find_label(
    db: &DatabaseConnection,
    label_id: Uuid,
) -> Result<Option<label::Model>, AppError> {
    label::Entity::find_by_id(label_id)
        .one(db)
        .await
        .map_err(map_db_err)
}

/// 更新标签名称/颜色。
pub async fn update_label(
    db: &DatabaseConnection,
    current: &label::Model,
    name: &str,
    color: &str,
) -> Result<label::Model, AppError> {
    let mut active: label::ActiveModel = current.clone().into();
    active.name = Set(name.to_string());
    active.color = Set(color.to_string());
    active.update(db).await.map_err(map_db_err)
}

/// 删除标签（同时清掉卡片关联）。
pub async fn delete_label(db: &DatabaseConnection, label_id: Uuid) -> Result<(), AppError> {
    task_label::Entity::delete_many()
        .filter(task_label::Column::LabelId.eq(label_id))
        .exec(db)
        .await
        .map_err(map_db_err)?;
    label::Entity::delete_by_id(label_id)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(map_db_err)
}

/// 给卡片打标签（幂等）。
pub async fn attach_label(
    db: &DatabaseConnection,
    task_id: Uuid,
    label_id: Uuid,
) -> Result<(), AppError> {
    let existing = task_label::Entity::find_by_id((task_id, label_id))
        .one(db)
        .await
        .map_err(map_db_err)?;
    if existing.is_some() {
        return Ok(());
    }
    task_label::ActiveModel {
        task_id: Set(task_id),
        label_id: Set(label_id),
    }
    .insert(db)
    .await
    .map(|_| ())
    .map_err(map_db_err)
}

/// 取消卡片标签。
pub async fn detach_label(
    db: &DatabaseConnection,
    task_id: Uuid,
    label_id: Uuid,
) -> Result<(), AppError> {
    task_label::Entity::delete_by_id((task_id, label_id))
        .exec(db)
        .await
        .map(|_| ())
        .map_err(map_db_err)
}

/// 卡片 → 标签映射。
pub async fn labels_for_tasks(
    db: &DatabaseConnection,
    task_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<label::Model>>, AppError> {
    let mut result: std::collections::HashMap<Uuid, Vec<label::Model>> =
        std::collections::HashMap::new();
    if task_ids.is_empty() {
        return Ok(result);
    }
    let links = task_label::Entity::find()
        .filter(task_label::Column::TaskId.is_in(task_ids.to_vec()))
        .all(db)
        .await
        .map_err(map_db_err)?;
    if links.is_empty() {
        return Ok(result);
    }
    let label_ids: Vec<Uuid> = links.iter().map(|row| row.label_id).collect();
    let labels = label::Entity::find()
        .filter(label::Column::Id.is_in(label_ids))
        .all(db)
        .await
        .map_err(map_db_err)?;
    let by_id: std::collections::HashMap<Uuid, label::Model> =
        labels.into_iter().map(|row| (row.id, row)).collect();
    for link in links {
        if let Some(label) = by_id.get(&link.label_id) {
            result.entry(link.task_id).or_default().push(label.clone());
        }
    }
    Ok(result)
}
