//! task schema v9：看板成员与角色覆盖（P1）。
//!
//! 默认继承项目角色（项目管理员=看板 admin，普通成员=editor），
//! 本表可覆盖为 viewer/editor/admin。

use sea_orm_migration::prelude::*;

/// v9 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE board_members (\
                 board_id uuid NOT NULL, \
                 user_id uuid NOT NULL, \
                 role text NOT NULL, \
                 joined_at timestamptz NOT NULL, \
                 PRIMARY KEY (board_id, user_id))",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
