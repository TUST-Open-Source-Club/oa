//! task schema v12：卡片依赖（P2）。

use sea_orm_migration::prelude::*;

/// v12 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE task_dependencies (\
                 task_id uuid NOT NULL, \
                 blocked_by_id uuid NOT NULL, \
                 PRIMARY KEY (task_id, blocked_by_id))",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_task_dependencies_blocker ON task_dependencies (blocked_by_id)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
