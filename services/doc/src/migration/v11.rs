//! doc schema v11：文档模板（P2）。

use sea_orm_migration::prelude::*;

/// v11 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE doc_templates (\
                 id uuid PRIMARY KEY, \
                 space_id uuid NOT NULL, \
                 name text NOT NULL, \
                 content_md text NOT NULL, \
                 created_by uuid NOT NULL, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_doc_templates_space ON doc_templates (space_id, created_at DESC)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
