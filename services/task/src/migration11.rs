//! task schema v11：卡片模板（P1）。

use sea_orm_migration::prelude::*;

/// v11 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE card_templates (\
                 id uuid PRIMARY KEY, \
                 board_id uuid NOT NULL, \
                 name text NOT NULL, \
                 title text NOT NULL, \
                 description_md text NOT NULL, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_card_templates_board ON card_templates (board_id, created_at DESC)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
