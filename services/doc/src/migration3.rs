//! doc schema v3：文档可见性与分享链接（P0）。
//!
//! - `visibility`：`space`（空间成员可读，默认）/ `private`（仅编辑者）；
//! - 分享：随机 token + 可选密码（argon2，盐用 token 本身）+ 可选过期时间 + 权限。

use sea_orm_migration::prelude::*;

/// v3 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE nodes ADD COLUMN visibility text NOT NULL DEFAULT 'space'",
        )
        .await?;
        conn.execute_unprepared("ALTER TABLE nodes ADD COLUMN share_token text")
            .await?;
        conn.execute_unprepared("ALTER TABLE nodes ADD COLUMN share_password_hash text")
            .await?;
        conn.execute_unprepared("ALTER TABLE nodes ADD COLUMN share_expires_at timestamptz")
            .await?;
        conn.execute_unprepared("ALTER TABLE nodes ADD COLUMN share_permission text")
            .await?;
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX ix_nodes_share_token ON nodes (share_token) \
             WHERE share_token IS NOT NULL",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
