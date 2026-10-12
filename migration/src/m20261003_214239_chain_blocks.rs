use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "chain_blocks",
            &[
                ("id", ColType::PkAuto),
                ("block_id", ColType::StringUniq),
                ("height", ColType::BigInteger),
                ("block_time", ColType::TimestampWithTimeZone),
                ("miner", ColType::String),
                ("bits", ColType::BigInteger),
                ("tuple", ColType::BigInteger),
                ("certified", ColType::Boolean),
                ("p", ColType::Text),
                ("txs", ColType::BigInteger),
                ("main_chain", ColType::Boolean),
            ],
            &[],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "chain_blocks").await
    }
}
