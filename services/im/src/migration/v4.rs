//! im schema v4：消息编辑历史（P1）。
//!
//! 每次编辑写入一条 `message_revisions`（编辑后的内容快照 + 版本号 + 编辑者），
//! 消息本体的 `edited_at` 标记最近一次编辑；原内容在首次编辑时补录 version=1。

use sea_orm_migration::prelude::*;

/// v4 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE message_revisions (\
                 id uuid PRIMARY KEY, \
                 message_id uuid NOT NULL, \
                 version int NOT NULL, \
                 content jsonb NOT NULL, \
                 editor_id uuid, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_message_revisions_message ON message_revisions (message_id, version)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
