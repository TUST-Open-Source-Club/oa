//! im schema v8：会话级通知规则（P1）。
//!
//! 每个成员对每个会话独立设置：`all`（全部）/ `mentions`（仅 @我）/ `mute`（静音），
//! 发送消息时据此裁剪 notify 的提醒对象。

use sea_orm_migration::prelude::*;

/// v8 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE conversation_members \
             ADD COLUMN notify_level text NOT NULL DEFAULT 'all'",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
