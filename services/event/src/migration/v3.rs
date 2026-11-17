//! 活动关联筹备项目/看板（P0 活动 × 任务连动）。

use sea_orm_migration::prelude::*;

/// v3 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for sql in [
            "ALTER TABLE events ADD COLUMN task_project_id uuid",
            "ALTER TABLE events ADD COLUMN task_board_id uuid",
            "ALTER TABLE events ADD COLUMN checklist_template text",
            "CREATE INDEX ix_events_task_project ON events (task_project_id)",
        ] {
            conn.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
