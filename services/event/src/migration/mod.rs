//! event schema 迁移（含 outbox）。

use sea_orm_migration::prelude::*;

/// v1：初始 schema。
pub mod v1;

/// v2：变更日志表（审计与撤销）。
pub mod v2;

/// 迁移入口。
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(v1::Migration), Box::new(v2::Migration)]
    }
}

/// events 表标识符。
#[derive(DeriveIden)]
pub enum Events {
    /// 表。
    Table,
    /// id。
    Id,
    /// slug。
    Slug,
    /// title。
    Title,
    /// description_md。
    DescriptionMd,
    /// location。
    Location,
    /// start_at。
    StartAt,
    /// end_at。
    EndAt,
    /// reg_start_at。
    RegStartAt,
    /// reg_end_at。
    RegEndAt,
    /// capacity。
    Capacity,
    /// waitlist_enabled。
    WaitlistEnabled,
    /// need_review。
    NeedReview,
    /// email_verify。
    EmailVerify,
    /// email_domains。
    EmailDomains,
    /// status。
    Status,
    /// created_by。
    CreatedBy,
    /// created_at。
    CreatedAt,
    /// updated_at。
    UpdatedAt,
}

/// event_admins 表标识符。
#[derive(DeriveIden)]
pub enum EventAdmins {
    /// 表。
    Table,
    /// event_id。
    EventId,
    /// user_id。
    UserId,
    /// joined_at。
    JoinedAt,
}

/// form_schemas 表标识符。
#[derive(DeriveIden)]
pub enum FormSchemas {
    /// 表。
    Table,
    /// id。
    Id,
    /// event_id。
    EventId,
    /// version。
    Version,
    /// schema。
    Schema,
    /// created_at。
    CreatedAt,
}

/// registrations 表标识符。
#[derive(DeriveIden)]
pub enum Registrations {
    /// 表。
    Table,
    /// id。
    Id,
    /// event_id。
    EventId,
    /// schema_version。
    SchemaVersion,
    /// user_id。
    UserId,
    /// name。
    Name,
    /// email。
    Email,
    /// phone。
    Phone,
    /// answers。
    Answers,
    /// status。
    Status,
    /// checkin_code。
    CheckinCode,
    /// checked_in_at。
    CheckedInAt,
    /// created_at。
    CreatedAt,
}

/// verification_codes 表标识符。
#[derive(DeriveIden)]
pub enum VerificationCodes {
    /// 表。
    Table,
    /// id。
    Id,
    /// event_id。
    EventId,
    /// email。
    Email,
    /// purpose。
    Purpose,
    /// code_hash。
    CodeHash,
    /// expires_at。
    ExpiresAt,
    /// attempts。
    Attempts,
    /// used_at。
    UsedAt,
    /// created_at。
    CreatedAt,
}
