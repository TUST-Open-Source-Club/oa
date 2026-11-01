//! drive schema 迁移。

use sea_orm_migration::prelude::*;

/// v1：初始 schema。
pub mod v1;

/// v2：shares 增加分享密码列。
pub mod v2;

/// v3：分片上传会话表。
pub mod v3;

/// v4：变更日志表（审计与撤销）。
pub mod v4;

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

/// shares 表标识符。
#[derive(DeriveIden)]
pub enum Shares {
    /// 表。
    Table,
    /// id。
    Id,
    /// node_id。
    NodeId,
    /// token。
    Token,
    /// permission。
    Permission,
    /// expires_at。
    ExpiresAt,
    /// max_downloads。
    MaxDownloads,
    /// download_count。
    DownloadCount,
    /// created_by。
    CreatedBy,
    /// created_at。
    CreatedAt,
    /// revoked_at。
    RevokedAt,
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
    /// name。
    Name,
    /// kind。
    Kind,
    /// storage_key。
    StorageKey,
    /// size。
    Size,
    /// mime。
    Mime,
    /// hash。
    Hash,
    /// created_by。
    CreatedBy,
    /// deleted_at。
    DeletedAt,
    /// created_at。
    CreatedAt,
    /// updated_at。
    UpdatedAt,
}
