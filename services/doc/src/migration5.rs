//! doc schema v5：文档图片/附件（P0）。
//!
//! 小文件（≤5MB）直接存库，配合长期签名 URL 供 Markdown 图片持久引用；
//! 大文件仍走 drive。

use sea_orm_migration::prelude::*;

/// v5 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE doc_files (\
                 id uuid PRIMARY KEY, \
                 uploader_id uuid NOT NULL, \
                 name text NOT NULL, \
                 mime text NOT NULL, \
                 size int NOT NULL, \
                 data bytea NOT NULL, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
