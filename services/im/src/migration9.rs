//! im schema v9：媒体文件（P1）。
//!
//! 小文件（≤5MB，图片/附件）直接存库，避免 IM 依赖额外对象存储；
//! 大文件仍走 drive 服务。下载用 HMAC/JWT 签名 URL，便于 `<img>` 直接引用。

use sea_orm_migration::prelude::*;

/// v9 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE im_files (\
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
