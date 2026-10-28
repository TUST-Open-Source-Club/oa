//! im schema v10：消息收藏（P2）。
//!
//! 收藏是「用户 × 消息」的个人标记，与会话置顶（`conversation_members.pinned`）不同。

use sea_orm_migration::prelude::*;

/// v10 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE message_stars (\
                 user_id uuid NOT NULL, \
                 message_id uuid NOT NULL, \
                 conversation_id uuid NOT NULL, \
                 created_at timestamptz NOT NULL, \
                 PRIMARY KEY (user_id, message_id))",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_message_stars_user ON message_stars (user_id, created_at DESC)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
