//! doc schema v9：文档关注（P1）。

use sea_orm_migration::prelude::*;

/// v9 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE node_follows (\
                 user_id uuid NOT NULL, \
                 node_id uuid NOT NULL, \
                 created_at timestamptz NOT NULL, \
                 PRIMARY KEY (user_id, node_id))",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_node_follows_node ON node_follows (node_id)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
