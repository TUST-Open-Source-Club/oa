//! doc schema 初始迁移（v1）。

use super::*;

/// 初始化迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // schema 由连接层保证（connect_with_schema 创建并设置 search_path），
        // 此处不再创建固定 schema，避免并行测试/多租户连接时同名 schema 冲突。

        manager
            .create_table(
                Table::create()
                    .table(Spaces::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Spaces::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Spaces::Name).string_len(64).not_null())
                    .col(ColumnDef::new(Spaces::Type).string_len(16).not_null())
                    .col(ColumnDef::new(Spaces::OwnerId).uuid().not_null())
                    .col(
                        ColumnDef::new(Spaces::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SpaceMembers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SpaceMembers::SpaceId).uuid().not_null())
                    .col(ColumnDef::new(SpaceMembers::UserId).uuid().not_null())
                    .col(ColumnDef::new(SpaceMembers::Role).string_len(16).not_null())
                    .col(
                        ColumnDef::new(SpaceMembers::JoinedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(SpaceMembers::SpaceId)
                            .col(SpaceMembers::UserId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Nodes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Nodes::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Nodes::SpaceId).uuid().not_null())
                    .col(ColumnDef::new(Nodes::ParentId).uuid())
                    .col(ColumnDef::new(Nodes::Kind).string_len(16).not_null())
                    .col(ColumnDef::new(Nodes::Title).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Nodes::Position)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Nodes::Version)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(Nodes::ContentMd)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(Nodes::UpdatedBy).uuid().not_null())
                    .col(ColumnDef::new(Nodes::DeletedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Nodes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Nodes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_doc_nodes_space_parent")
                    .table(Nodes::Table)
                    .col(Nodes::SpaceId)
                    .col(Nodes::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PageVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageVersions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PageVersions::NodeId).uuid().not_null())
                    .col(
                        ColumnDef::new(PageVersions::Version)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageVersions::ContentMd).text().not_null())
                    .col(ColumnDef::new(PageVersions::AuthorId).uuid().not_null())
                    .col(
                        ColumnDef::new(PageVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_doc_versions_node_version")
                    .table(PageVersions::Table)
                    .col(PageVersions::NodeId)
                    .col(PageVersions::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageVersions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Nodes::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SpaceMembers::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Spaces::Table).if_exists().to_owned())
            .await?;
        Ok(())
    }
}
