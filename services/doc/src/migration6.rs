//! doc schema v6：块级评论（P1）。
//!
//! 评论锚定到文档内的标题/块（`anchor` 存 `{ heading }`），支持一级回复与“解决”状态。

use sea_orm_migration::prelude::*;

/// v6 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE node_comments (\
                 id uuid PRIMARY KEY, \
                 space_id uuid NOT NULL, \
                 node_id uuid NOT NULL, \
                 parent_id uuid, \
                 author_id uuid NOT NULL, \
                 anchor jsonb, \
                 body text NOT NULL, \
                 resolved_at timestamptz, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_node_comments_node ON node_comments (node_id, created_at)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_node_comments_parent ON node_comments (parent_id)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
