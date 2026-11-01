//! doc schema 迁移。

use sea_orm_migration::prelude::*;

/// v1：初始 schema。
pub mod v1;

/// v2：变更日志表（审计与撤销）。
pub mod v2;

/// v3：文档可见性与分享链接（P0）。
pub mod v3;

/// v4：文档搜索索引（P0）。
pub mod v4;

/// v5：文档图片/附件（P0）。
pub mod v5;

/// v6：块级评论（P1）。
pub mod v6;

/// v7：空间所有者角色（P1）。
pub mod v7;

/// v8：outbox（通知事件）。
pub mod v8;

/// v9：文档关注（P1）。
pub mod v9;

/// v10：阅读记录（P2）。
pub mod v10;

/// v11：文档模板（P2）。
pub mod v11;

/// v12：文档互链（P2）。
pub mod v12;

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
        ]
    }
}

/// spaces 表标识符。
#[derive(DeriveIden)]
pub enum Spaces {
    /// 表。
    Table,
    /// id。
    Id,
    /// name。
    Name,
    /// type。
    Type,
    /// owner_id。
    OwnerId,
    /// created_at。
    CreatedAt,
}

/// space_members 表标识符。
#[derive(DeriveIden)]
pub enum SpaceMembers {
    /// 表。
    Table,
    /// space_id。
    SpaceId,
    /// user_id。
    UserId,
    /// role。
    Role,
    /// joined_at。
    JoinedAt,
}

/// nodes 表标识符。
#[derive(DeriveIden)]
pub enum Nodes {
    /// 表。
    Table,
    /// id。
    Id,
    /// space_id。
    SpaceId,
    /// parent_id。
    ParentId,
    /// kind。
    Kind,
    /// title。
    Title,
    /// position。
    Position,
    /// version。
    Version,
    /// content_md。
    ContentMd,
    /// updated_by。
    UpdatedBy,
    /// deleted_at。
    DeletedAt,
    /// created_at。
    CreatedAt,
    /// updated_at。
    UpdatedAt,
}

/// page_versions 表标识符。
#[derive(DeriveIden)]
pub enum PageVersions {
    /// 表。
    Table,
    /// id。
    Id,
    /// node_id。
    NodeId,
    /// version。
    Version,
    /// content_md。
    ContentMd,
    /// author_id。
    AuthorId,
    /// created_at。
    CreatedAt,
}
