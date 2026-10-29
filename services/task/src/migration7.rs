//! task schema v7：卡片归档（P0）。

use sea_orm_migration::prelude::*;

/// v7 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("ALTER TABLE tasks ADD COLUMN archived_at timestamptz")
            .await?;
        conn.execute_unprepared("CREATE INDEX ix_tasks_archived ON tasks (column_id, archived_at)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
