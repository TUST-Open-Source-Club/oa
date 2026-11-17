//! 看板自动化规则（P2）。

use sea_orm_migration::prelude::*;

/// v16 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE automation_rules (\
                 id uuid PRIMARY KEY, \
                 board_id uuid NOT NULL, \
                 trigger text NOT NULL, \
                 action text NOT NULL, \
                 label_id uuid, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_automation_rules_board ON automation_rules (board_id, created_at)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
