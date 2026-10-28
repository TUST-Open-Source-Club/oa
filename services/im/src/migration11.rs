//! im schema v11：审计表（P2）。
//!
//! 复用 `club_bus::audit` 的 `change_log` 结构，用于记录导出等敏感操作。

use sea_orm_migration::prelude::*;

/// v11 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(club_bus::audit::DDL)
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
