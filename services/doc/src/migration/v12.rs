//! doc schema v12：文档互链（P2）。
//!
//! 正文里的 `[[标题]]` 在保存时解析为同空间的页面引用，用于「反向链接」。

use sea_orm_migration::prelude::*;

/// v12 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE doc_links (\
                 from_node_id uuid NOT NULL, \
                 to_node_id uuid NOT NULL, \
                 space_id uuid NOT NULL, \
                 created_at timestamptz NOT NULL, \
                 PRIMARY KEY (from_node_id, to_node_id))",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_doc_links_to ON doc_links (to_node_id)")
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
