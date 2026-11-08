//! 工时记录与预估工时（P2）。

use sea_orm_migration::prelude::*;

/// v14 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("ALTER TABLE tasks ADD COLUMN estimated_minutes int")
            .await?;
        conn.execute_unprepared(
            "CREATE TABLE task_time_entries (\
                 id uuid PRIMARY KEY, \
                 task_id uuid NOT NULL, \
                 user_id uuid NOT NULL, \
                 minutes int NOT NULL, \
                 note text NOT NULL DEFAULT '', \
                 spent_on date NOT NULL, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_task_time_entries_task ON task_time_entries (task_id, created_at)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
