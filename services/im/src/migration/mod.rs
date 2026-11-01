//! im schema 迁移（含 outbox 表，事件投递用）。

use sea_orm_migration::prelude::*;

/// v1：初始 schema。
pub mod v1;

/// v2：群管理（成员禁言/发言白名单、仅管理员发言）。
pub mod v2;

/// v3：消息 `@` 提及与编辑时间（P0）。
pub mod v3;

/// v4：消息编辑历史（P1）。
pub mod v4;

/// v5：消息线程（P1）。
pub mod v5;

/// v6：消息表情回应（P1）。
pub mod v6;

/// v7：消息搜索索引（P1）。
pub mod v7;

/// v8：会话级通知规则（P1）。
pub mod v8;

/// v9：媒体文件（P1）。
pub mod v9;

/// v10：消息收藏（P2）。
pub mod v10;

/// v11：审计表（P2）。
pub mod v11;

/// 迁移入口。
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(v1::Migration),
            Box::new(v2::Migration),
            Box::new(v3::Migration),
            Box::new(v4::Migration),
            Box::new(v5::Migration),
            Box::new(v6::Migration),
            Box::new(v7::Migration),
            Box::new(v8::Migration),
            Box::new(v9::Migration),
            Box::new(v10::Migration),
            Box::new(v11::Migration),
        ]
    }
}
