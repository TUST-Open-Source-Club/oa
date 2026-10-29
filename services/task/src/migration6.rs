//! task schema v6：看板标签（P0）。

use sea_orm_migration::prelude::*;

/// v6 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE labels (\
                 id uuid PRIMARY KEY, \
                 board_id uuid NOT NULL, \
                 name text NOT NULL, \
                 color text NOT NULL, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_labels_board ON labels (board_id)")
            .await?;
        conn.execute_unprepared(
            "CREATE TABLE task_labels (\
                 task_id uuid NOT NULL, \
                 label_id uuid NOT NULL, \
                 PRIMARY KEY (task_id, label_id))",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_task_labels_label ON task_labels (label_id)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
