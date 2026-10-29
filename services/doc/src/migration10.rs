//! doc schema v10：阅读记录（P2）。
//!
//! 按「用户 × 文档」去重记录最近浏览时间，用于详情页浏览数与会话侧栏「最近浏览」。

use sea_orm_migration::prelude::*;

/// v10 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE node_views (\
                 user_id uuid NOT NULL, \
                 node_id uuid NOT NULL, \
                 space_id uuid NOT NULL, \
                 viewed_at timestamptz NOT NULL, \
                 PRIMARY KEY (user_id, node_id))",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_node_views_user ON node_views (user_id, viewed_at DESC)",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_node_views_node ON node_views (node_id)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
