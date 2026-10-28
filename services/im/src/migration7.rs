//! im schema v7：消息搜索索引（P1）。
//!
//! 优先启用 `pg_trgm` 与 GIN 索引加速 `ILIKE '%关键词%'`；数据库无该扩展或权限不足时
//! 降级为顺序扫描（搜索仍可用，只是大表较慢），不让迁移失败挡住启动。
//!
//! 两个坑：
//! 1. 并行测试/多 schema 部署会并发执行本迁移，`CREATE EXTENSION IF NOT EXISTS`
//!    仍可能因唯一约束竞争失败；这里用事务级 advisory lock 串行化，并在建前查一次
//!    `pg_extension`，避免在已安装时重复创建。
//! 2. 迁移跑在事务里，任何语句失败都会让事务进入 aborted 状态；因此扩展与索引的
//!    创建都包在 SAVEPOINT 里，失败时 `ROLLBACK TO SAVEPOINT` 后继续。
//!
//! 扩展可能装在别的 schema（多 schema 共享库），索引里的 operator class 需要按
//! `pg_extension.extnamespace` 显式限定，否则 `gin_trgm_ops` 不在 search_path 上。

use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

/// 迁移用的 advisory lock key（本服务的固定常量）。
const LOCK_KEY: i64 = 7_391_863_151;

/// v7 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

impl Migration {
    /// 在 savepoint 内执行语句；失败时回滚到 savepoint 并返回 false。
    async fn try_in_savepoint(conn: &SchemaManagerConnection<'_>, sql: &str) -> bool {
        if let Err(err) = conn.execute_unprepared("SAVEPOINT im_trgm").await {
            tracing::warn!(error = %err, "创建 savepoint 失败");
            return false;
        }
        match conn.execute_unprepared(sql).await {
            Ok(_) => {
                let _ = conn.execute_unprepared("RELEASE SAVEPOINT im_trgm").await;
                true
            }
            Err(err) => {
                tracing::warn!(error = %err, sql, "语句失败，已回滚到 savepoint");
                let _ = conn
                    .execute_unprepared("ROLLBACK TO SAVEPOINT im_trgm")
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
        // 并发迁移时串行化扩展安装（事务级锁，提交后自动释放）
        conn.execute_unprepared(&format!("SELECT pg_advisory_xact_lock({LOCK_KEY})"))
            .await?;
        let installed = conn
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm'".to_string(),
            ))
            .await?;
        if installed.is_none() && !Self::try_in_savepoint(conn, "CREATE EXTENSION pg_trgm").await {
            tracing::warn!("pg_trgm 不可用，搜索退化为顺序扫描");
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
            tracing::warn!("pg_trgm 未安装，搜索退化为顺序扫描");
            return Ok(());
        };
        let schema: String = row.try_get_by_index(0)?;
        let ddl = format!(
            "CREATE INDEX IF NOT EXISTS ix_messages_text_trgm ON messages \
             USING gin ((content->>'text') \"{schema}\".gin_trgm_ops)"
        );
        if !Self::try_in_savepoint(conn, &ddl).await {
            tracing::warn!("创建 trigram 索引失败，搜索退化为顺序扫描");
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
