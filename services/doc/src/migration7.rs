//! doc schema v7：空间所有者角色（P1）。
//!
//! 空间创建者由 `admin` 升级为 `owner`（唯一），后续成员管理权限围绕它收敛。

use sea_orm_migration::prelude::*;

/// v7 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE space_members m SET role = 'owner' \
                 FROM spaces s \
                 WHERE m.space_id = s.id AND m.user_id = s.owner_id AND m.role = 'admin'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
