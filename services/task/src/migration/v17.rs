//! 任务关联活动（P0：活动 × 任务连动）。

use sea_orm_migration::prelude::*;

/// v17 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("ALTER TABLE tasks ADD COLUMN event_id uuid")
            .await?;
        conn.execute_unprepared("CREATE INDEX ix_tasks_event ON tasks (event_id)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
