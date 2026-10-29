//! task schema v5：项目 → 多看板（P0）。
//!
//! 每个项目至少一个「默认看板」；既有列迁移到默认看板，列的 `project_id` 保留兼容。

use sea_orm_migration::prelude::*;

/// v5 迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "CREATE TABLE boards (\
                 id uuid PRIMARY KEY, \
                 project_id uuid NOT NULL, \
                 name text NOT NULL, \
                 position bigint NOT NULL, \
                 archived_at timestamptz, \
                 created_at timestamptz NOT NULL)",
        )
        .await?;
        conn.execute_unprepared("CREATE INDEX ix_boards_project ON boards (project_id, position)")
            .await?;
        // 每个项目补一个默认看板
        conn.execute_unprepared(
            "INSERT INTO boards (id, project_id, name, position, archived_at, created_at) \
             SELECT gen_random_uuid(), p.id, '默认看板', 0, NULL, now() FROM projects p",
        )
        .await?;
        conn.execute_unprepared("ALTER TABLE board_columns ADD COLUMN board_id uuid")
            .await?;
        conn.execute_unprepared(
            "UPDATE board_columns c SET board_id = b.id \
             FROM boards b WHERE b.project_id = c.project_id",
        )
        .await?;
        conn.execute_unprepared("ALTER TABLE board_columns ALTER COLUMN board_id SET NOT NULL")
            .await?;
        conn.execute_unprepared(
            "CREATE INDEX ix_board_columns_board ON board_columns (board_id, position)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
