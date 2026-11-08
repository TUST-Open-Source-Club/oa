//! task schema 迁移（含 outbox 表）。

use sea_orm_migration::prelude::*;

/// v1：初始 schema。
pub mod v1;

/// v2：任务开始时间、状态（active/done/terminated）、多负责人表与回填。
pub mod v2;

/// v3：任务附件表。
pub mod v3;

/// v4：变更日志表（审计与撤销）。
pub mod v4;

/// v5：项目 → 多看板（P0）。
pub mod v5;

/// v6：看板标签（P0）。
pub mod v6;

/// v7：卡片归档（P0）。
pub mod v7;

/// v8：卡片封面（P0）。
pub mod v8;

/// v9：看板成员与角色覆盖（P1）。
pub mod v9;

/// v10：到期提醒记录（P1）。
pub mod v10;

/// v11：卡片模板（P1）。
pub mod v11;

/// v12：卡片依赖（P2）。
pub mod v12;

/// v13：列 WIP 限制（P2）。
pub mod v13;

/// v14：工时记录与预估工时（P2）。
pub mod v14;

/// 迁移入口。
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(v1::Migration),
            Box::new(v2::Migration),
            Box::new(v3::Migration),
            Box::new(v4::Migration),
            Box::new(v5::Migration),
            Box::new(v6::Migration),
            Box::new(v7::Migration),
            Box::new(v8::Migration),
            Box::new(v9::Migration),
            Box::new(v10::Migration),
            Box::new(v11::Migration),
            Box::new(v12::Migration),
            Box::new(v13::Migration),
            Box::new(v14::Migration),
        ]
    }
}

/// projects 表标识符。
#[derive(DeriveIden)]
pub enum Projects {
    /// 表。
    Table,
    /// id。
    Id,
    /// name。
    Name,
    /// description。
    Description,
    /// owner_id。
    OwnerId,
    /// archived_at。
    ArchivedAt,
    /// created_at。
    CreatedAt,
}

/// project_members 表标识符。
#[derive(DeriveIden)]
pub enum ProjectMembers {
    /// 表。
    Table,
    /// project_id。
    ProjectId,
    /// user_id。
    UserId,
    /// role。
    Role,
    /// joined_at。
    JoinedAt,
}

/// board_columns 表标识符。
#[derive(DeriveIden)]
pub enum BoardColumns {
    /// 表。
    Table,
    /// id。
    Id,
    /// project_id。
    ProjectId,
    /// name。
    Name,
    /// position。
    Position,
    /// is_done。
    IsDone,
    /// created_at。
    CreatedAt,
}

/// tasks 表标识符。
#[derive(DeriveIden)]
pub enum Tasks {
    /// 表。
    Table,
    /// id。
    Id,
    /// project_id。
    ProjectId,
    /// column_id。
    ColumnId,
    /// title。
    Title,
    /// description_md。
    DescriptionMd,
    /// assignee_id。
    AssigneeId,
    /// priority。
    Priority,
    /// due_at。
    DueAt,
    /// position。
    Position,
    /// created_by。
    CreatedBy,
    /// completed_at。
    CompletedAt,
    /// created_at。
    CreatedAt,
    /// updated_at。
    UpdatedAt,
}

/// subtasks 表标识符。
#[derive(DeriveIden)]
pub enum Subtasks {
    /// 表。
    Table,
    /// id。
    Id,
    /// task_id。
    TaskId,
    /// title。
    Title,
    /// done。
    Done,
    /// position。
    Position,
    /// created_at。
    CreatedAt,
}

/// task_comments 表标识符。
#[derive(DeriveIden)]
pub enum TaskComments {
    /// 表。
    Table,
    /// id。
    Id,
    /// task_id。
    TaskId,
    /// author_id。
    AuthorId,
    /// content_md。
    ContentMd,
    /// created_at。
    CreatedAt,
}
