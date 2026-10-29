//! doc schema v8：outbox（通知事件）。
//!
//! 与其它服务一致：事件先落库，由后台投递到 Redis Streams。

use sea_orm_migration::prelude::*;

/// v8 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(club_bus::outbox::DDL)
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
