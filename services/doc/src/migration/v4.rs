//! doc schema v4：文档搜索索引（P0）。
//!
//! 启用 `pg_trgm` 并给标题/正文建 GIN 索引；无扩展权限时降级为顺序扫描。
//! 多 schema 场景下 operator class 需按 `pg_extension.extnamespace` 限定，
//! 且语句失败要用 SAVEPOINT 回滚，避免把迁移事务打挂。

use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

/// 迁移用 advisory lock key（与 im 的区分开）。
const LOCK_KEY: i64 = 7_391_863_152;

/// v4 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

impl Migration {
    /// 在 savepoint 内执行语句；失败回滚并返回 false。
    async fn try_in_savepoint(conn: &SchemaManagerConnection<'_>, sql: &str) -> bool {
        if conn.execute_unprepared("SAVEPOINT doc_trgm").await.is_err() {
            return false;
        }
        match conn.execute_unprepared(sql).await {
            Ok(_) => {
                let _ = conn.execute_unprepared("RELEASE SAVEPOINT doc_trgm").await;
                true
            }
            Err(err) => {
                tracing::warn!(error = %err, sql, "语句失败，已回滚到 savepoint");
                let _ = conn
                    .execute_unprepared("ROLLBACK TO SAVEPOINT doc_trgm")
                    .await;
                false
            }
        }
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(&format!("SELECT pg_advisory_xact_lock({LOCK_KEY})"))
            .await?;
        let installed = conn
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm'".to_string(),
            ))
            .await?;
        if installed.is_none() && !Self::try_in_savepoint(conn, "CREATE EXTENSION pg_trgm").await {
            tracing::warn!("pg_trgm 不可用，文档搜索退化为顺序扫描");
            return Ok(());
        }
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
            return Ok(());
        };
        let schema: String = row.try_get_by_index(0)?;
        for sql in [
            format!(
                "CREATE INDEX IF NOT EXISTS ix_nodes_title_trgm ON nodes \
                 USING gin (title \"{schema}\".gin_trgm_ops)"
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS ix_nodes_content_trgm ON nodes \
                 USING gin (content_md \"{schema}\".gin_trgm_ops)"
            ),
        ] {
            if !Self::try_in_savepoint(conn, &sql).await {
                tracing::warn!("创建文档搜索索引失败，退化为顺序扫描");
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
