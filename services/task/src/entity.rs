//! task schema 实体。

/// 项目。
pub mod project {
    use sea_orm::entity::prelude::*;

    /// 项目模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projects")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 名称。
        pub name: String,
        /// 描述。
        #[sea_orm(column_type = "Text")]
        pub description: String,
        /// 拥有者。
        pub owner_id: Uuid,
        /// 归档时间。
        #[sea_orm(nullable)]
        pub archived_at: Option<DateTimeWithTimeZone>,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 看板（项目下可多个）。
pub mod board {
    use sea_orm::entity::prelude::*;

    /// 看板模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "boards")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 所属项目。
        pub project_id: Uuid,
        /// 名称。
        pub name: String,
        /// 排序。
        pub position: i64,
        /// 归档时间。
        #[sea_orm(nullable)]
        pub archived_at: Option<DateTimeWithTimeZone>,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 项目成员。
pub mod project_member {
    use sea_orm::entity::prelude::*;

    /// 管理员。
    pub const ROLE_ADMIN: &str = "admin";
    /// 成员。
    pub const ROLE_MEMBER: &str = "member";

    /// 成员模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "project_members")]
    pub struct Model {
        /// 项目。
        #[sea_orm(primary_key, auto_increment = false)]
        pub project_id: Uuid,
        /// 用户。
        #[sea_orm(primary_key, auto_increment = false)]
        pub user_id: Uuid,
        /// 角色。
        pub role: String,
        /// 加入时间。
        pub joined_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 看板列。
pub mod column {
    use sea_orm::entity::prelude::*;

    /// 看板列模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "board_columns")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 项目。
        pub project_id: Uuid,
        /// 名称。
        pub name: String,
        /// 所属看板。
        pub board_id: Uuid,
        /// 排序。
        pub position: i64,
        /// 是否为完成列。
        pub is_done: bool,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 任务。
pub mod task {
    use sea_orm::entity::prelude::*;

    /// 任务模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "tasks")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 项目。
        pub project_id: Uuid,
        /// 所在列。
        pub column_id: Uuid,
        /// 标题。
        pub title: String,
        /// 描述（Markdown）。
        #[sea_orm(column_type = "Text")]
        pub description_md: String,
        /// 负责人（多负责人取首个，保持兼容）。
        #[sea_orm(nullable)]
        pub assignee_id: Option<Uuid>,
        /// 开始时间。
        #[sea_orm(nullable)]
        pub start_at: Option<DateTimeWithTimeZone>,
        /// 优先级：low/normal/high/urgent。
        pub priority: String,
        /// 截止时间。
        #[sea_orm(nullable)]
        pub due_at: Option<DateTimeWithTimeZone>,
        /// 列内排序。
        pub position: i64,
        /// 创建者。
        pub created_by: Uuid,
        /// 状态：active/done/terminated。
        pub status: String,
        /// 完成时间。
        #[sea_orm(nullable)]
        pub completed_at: Option<DateTimeWithTimeZone>,
        /// 封面附件。
        #[sea_orm(nullable)]
        pub cover_attachment_id: Option<Uuid>,
        /// 到期提醒时间。
        #[sea_orm(nullable)]
        pub due_reminded_at: Option<DateTimeWithTimeZone>,
        /// 归档时间。
        #[sea_orm(nullable)]
        pub archived_at: Option<DateTimeWithTimeZone>,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
        /// 更新时间。
        pub updated_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 任务负责人（多对多）。
pub mod task_assignee {
    use sea_orm::entity::prelude::*;

    /// 负责人模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "task_assignees")]
    pub struct Model {
        /// 任务。
        #[sea_orm(primary_key, auto_increment = false)]
        pub task_id: Uuid,
        /// 用户。
        #[sea_orm(primary_key, auto_increment = false)]
        pub user_id: Uuid,
        /// 指派时间。
        pub assigned_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 任务附件。
pub mod task_attachment {
    use sea_orm::entity::prelude::*;

    /// 附件模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "task_attachments")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 任务。
        pub task_id: Uuid,
        /// 原始文件名。
        pub name: String,
        /// 字节数。
        pub size: i64,
        /// MIME。
        pub mime: String,
        /// 存储相对路径（{task_id}/{attachment_id}）。
        pub storage_key: String,
        /// 上传者。
        pub uploader_id: Uuid,
        /// 上传时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 子任务。
pub mod subtask {
    use sea_orm::entity::prelude::*;

    /// 子任务模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "subtasks")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 任务。
        pub task_id: Uuid,
        /// 标题。
        pub title: String,
        /// 是否完成。
        pub done: bool,
        /// 排序。
        pub position: i64,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 任务评论。
pub mod comment {
    use sea_orm::entity::prelude::*;

    /// 评论模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "task_comments")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 任务。
        pub task_id: Uuid,
        /// 作者。
        pub author_id: Uuid,
        /// 内容（Markdown）。
        #[sea_orm(column_type = "Text")]
        pub content_md: String,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 看板标签。
pub mod label {
    use sea_orm::entity::prelude::*;

    /// 标签模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "labels")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 所属看板。
        pub board_id: Uuid,
        /// 名称。
        pub name: String,
        /// 颜色（#rrggbb）。
        pub color: String,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 卡片标签关联。
pub mod task_label {
    use sea_orm::entity::prelude::*;

    /// 关联模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "task_labels")]
    pub struct Model {
        /// 任务。
        #[sea_orm(primary_key, auto_increment = false)]
        pub task_id: Uuid,
        /// 标签。
        #[sea_orm(primary_key, auto_increment = false)]
        pub label_id: Uuid,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 看板成员（角色覆盖）。
pub mod board_member {
    use sea_orm::entity::prelude::*;

    /// 看板成员模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "board_members")]
    pub struct Model {
        /// 看板。
        #[sea_orm(primary_key, auto_increment = false)]
        pub board_id: Uuid,
        /// 用户。
        #[sea_orm(primary_key, auto_increment = false)]
        pub user_id: Uuid,
        /// admin / editor / viewer。
        pub role: String,
        /// 加入时间。
        pub joined_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 卡片模板。
pub mod card_template {
    use sea_orm::entity::prelude::*;

    /// 模板模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "card_templates")]
    pub struct Model {
        /// ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 所属看板。
        pub board_id: Uuid,
        /// 模板名称。
        pub name: String,
        /// 卡片标题。
        pub title: String,
        /// 卡片描述。
        #[sea_orm(column_type = "Text")]
        pub description_md: String,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 卡片依赖（task 依赖 blocked_by）。
pub mod task_dependency {
    use sea_orm::entity::prelude::*;

    /// 依赖模型。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "task_dependencies")]
    pub struct Model {
        /// 任务。
        #[sea_orm(primary_key, auto_increment = false)]
        pub task_id: Uuid,
        /// 前置任务。
        #[sea_orm(primary_key, auto_increment = false)]
        pub blocked_by_id: Uuid,
    }

    /// 关系。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
