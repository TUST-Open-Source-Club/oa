//! im schema v7：消息搜索索引（P1）。
//!
//! 优先启用 `pg_trgm` 与 GIN 索引加速 `ILIKE '%关键词%'`；数据库无该扩展或权限不足时
//! 降级为顺序扫描（搜索仍可用，只是大表较慢），不让迁移失败挡住启动。
//!
//! 注意：多 schema 部署/并行测试下扩展只需安装一次，后续 schema 里
//! `gin_trgm_ops` 不在 search_path 上，因此索引里的 operator class 要按
//! `pg_extension.extnamespace` 显式限定。

use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

/// v7 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        if let Err(err) = conn
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await
        {
            tracing::warn!(error = %err, "启用 pg_trgm 失败，搜索退化为顺序扫描");
            return Ok(());
        }
        // 扩展可能装在别的 schema（并行测试/共享库），取出真实 schema 名再限定 opclass
        let row = conn
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT n.nspname FROM pg_extension e \
                 JOIN pg_namespace n ON n.oid = e.extnamespace \
                 WHERE e.extname = 'pg_trgm'"
                    .to_string(),
            ))
            .await?;
        let Some(row) = row else {
            tracing::warn!("pg_trgm 未安装，搜索退化为顺序扫描");
            return Ok(());
        };
        let schema: String = row.try_get_by_index(0)?;
        let ddl = format!(
            "CREATE INDEX IF NOT EXISTS ix_messages_text_trgm ON messages \
             USING gin ((content->>'text') \"{schema}\".gin_trgm_ops)"
        );
        if let Err(err) = conn.execute_unprepared(&ddl).await {
            tracing::warn!(error = %err, "创建 trigram 索引失败，搜索退化为顺序扫描");
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
