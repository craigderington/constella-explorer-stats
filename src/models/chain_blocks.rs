pub use super::_entities::chain_blocks::{ActiveModel, Entity, Model};

use super::_entities::chain_blocks::Column;
use crate::data::explorer::ExplorerBlock;
use loco_rs::prelude::*;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use std::collections::HashSet;

pub type ChainBlocks = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

// implement your read-oriented logic here
impl Model {
    /// The most recent `limit` blocks by height, newest first.
    ///
    /// # Errors
    /// On database failure.
    pub async fn recent<C>(db: &C, limit: u64) -> ModelResult<Vec<Self>>
    where
        C: ConnectionTrait,
    {
        Ok(Entity::find()
            .order_by_desc(Column::Height)
            .limit(limit)
            .all(db)
            .await?)
    }
}

// implement your write-oriented logic here
impl Model {
    /// Store blocks not seen before, deduplicated by block hash. Returns how
    /// many rows were inserted. Blocks whose timestamp will not parse are
    /// skipped rather than guessed at. Repeated IDs in the response and
    /// concurrent inserts are ignored using the unique `block_id` index.
    ///
    /// # Errors
    /// On database failure.
    pub async fn insert_new(
        db: &DatabaseConnection,
        blocks: &[ExplorerBlock],
    ) -> ModelResult<usize> {
        if blocks.is_empty() {
            return Ok(0);
        }

        let wanted: Vec<String> = blocks.iter().map(|b| b.id.clone()).collect();
        let known: HashSet<String> = Entity::find()
            .filter(Column::BlockId.is_in(wanted))
            .all(db)
            .await?
            .into_iter()
            .map(|m| m.block_id)
            .collect();
        let mut seen = known;

        let mut inserted = 0;
        for b in blocks {
            if !seen.insert(b.id.clone()) {
                continue;
            }
            let Some(block_time) = b.block_time() else {
                tracing::warn!(block = %b.id, "skipping block with unparseable time");
                continue;
            };
            let result = Entity::insert(ActiveModel {
                block_id: Set(b.id.clone()),
                height: Set(b.height),
                block_time: Set(block_time),
                miner: Set(b.miner.clone()),
                bits: Set(i64::from(b.bits)),
                tuple: Set(i64::from(b.tuple)),
                certified: Set(b.certified),
                p: Set(b.p.clone()),
                txs: Set(b.txs),
                main_chain: Set(b.main),
                ..Default::default()
            })
            .on_conflict_do_nothing_on([Column::BlockId])
            .exec(db)
            .await?;
            if matches!(result, sea_orm::TryInsertResult::Inserted(_)) {
                inserted += 1;
            }
        }
        Ok(inserted)
    }
}

// implement your custom finders, selectors oriented logic here
impl Entity {}
