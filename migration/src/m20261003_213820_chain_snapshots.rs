use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "chain_snapshots",
            &[
                ("id", ColType::PkAuto),
                ("ok", ColType::Boolean),
                ("error", ColType::TextNull),
                ("network", ColType::StringNull),
                ("chain_id", ColType::StringNull),
                ("height", ColType::BigIntegerNull),
                ("tip", ColType::StringNull),
                ("bits", ColType::BigIntegerNull),
                ("blocks", ColType::BigIntegerNull),
                ("txs", ColType::BigIntegerNull),
                ("miners", ColType::BigIntegerNull),
                ("shares_per_min", ColType::DoubleNull),
                ("escrow", ColType::BigIntegerNull),
                ("sci_paid", ColType::BigIntegerNull),
                ("sci_claims", ColType::BigIntegerNull),
                ("ledger_check", ColType::StringNull),
                ("ledger_check_height", ColType::BigIntegerNull),
                ("peer", ColType::BooleanNull),
                ("last_block_at", ColType::TimestampWithTimeZoneNull),
                ("stats_updated_at", ColType::TimestampWithTimeZoneNull),
            ],
            &[],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "chain_snapshots").await
    }
}
