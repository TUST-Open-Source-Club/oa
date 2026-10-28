//! im schema v6：消息表情回应（P1）。
//!
//! 一条消息上同一用户同一表情只记一次（联合主键），点第二次即取消。

use sea_orm_migration::prelude::*;

/// v6 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE message_reactions (\
                 message_id uuid NOT NULL, \
                 user_id uuid NOT NULL, \
                 emoji text NOT NULL, \
                 created_at timestamptz NOT NULL, \
                 PRIMARY KEY (message_id, user_id, emoji))",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_message_reactions_message ON message_reactions (message_id)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
