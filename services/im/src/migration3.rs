//! im schema v3：消息 `@` 提及与编辑时间（P0）。
//!
//! - `mentions`：被提及用户 ID 的 jsonb 数组，便于 `@>` 过滤与 GIN 索引；
//!   含 `@所有人` 时写入特殊标记 `"@all"`（由服务端校验群主/管理员后才落库）。
//! - `edited_at`：消息编辑时间（P0 先落库，P1 的编辑历史复用该字段）。

use sea_orm_migration::prelude::*;

/// v3 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE messages ADD COLUMN mentions jsonb NOT NULL DEFAULT '[]'::jsonb",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS ix_messages_mentions ON messages USING gin (mentions jsonb_path_ops)",
        )
        .await?;
        conn.execute_unprepared("ALTER TABLE messages ADD COLUMN edited_at timestamptz")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
