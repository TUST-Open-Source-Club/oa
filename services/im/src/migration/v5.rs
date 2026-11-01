//! im schema v5：消息线程（P1）。
//!
//! `messages.thread_root_id` 指向线程根消息；为 NULL 表示主消息流中的消息。
//! 线程回复不再嵌套（根的 `thread_root_id` 必须为 NULL，由应用层校验）。

use sea_orm_migration::prelude::*;

/// v5 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("ALTER TABLE messages ADD COLUMN thread_root_id uuid")
            .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_messages_thread_root ON messages (thread_root_id) \
             WHERE thread_root_id IS NOT NULL",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
