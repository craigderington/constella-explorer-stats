use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "chain_alerts",
            &[
                ("id", ColType::PkAuto),
                ("alert_type", ColType::String),
                ("detail", ColType::Text),
                ("delivered_at", ColType::TimestampWithTimeZoneNull),
            ],
            &[("chain_snapshots", "chain_snapshots")],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "chain_alerts").await
    }
}
