use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use ts_rs::TS;

use crate::{
    data::explorer::Settings,
    models::_entities::{chain_blocks, chain_snapshots},
};

/// One recorded poll of the explorer — a point on the chain's timeline.
/// Amounts (`escrow`, `sci_paid`) are atoms: 1e-8 of a coin.
#[derive(serde::Serialize, serde::Deserialize, TS)]
pub struct SnapshotDto {
    #[ts(type = "number")]
    pub id: i64,
    /// Whether the poll itself succeeded.
    pub ok: bool,
    pub error: Option<String>,
    pub network: Option<String>,
    pub chain_id: Option<String>,
    #[ts(type = "number")]
    pub height: Option<i64>,
    pub tip: Option<String>,
    #[ts(type = "number")]
    pub bits: Option<i64>,
    #[ts(type = "number")]
    pub blocks: Option<i64>,
    #[ts(type = "number")]
    pub txs: Option<i64>,
    #[ts(type = "number")]
    pub miners: Option<i64>,
    pub shares_per_min: Option<f64>,
    #[ts(type = "number")]
    pub escrow: Option<i64>,
    #[ts(type = "number")]
    pub sci_paid: Option<i64>,
    #[ts(type = "number")]
    pub sci_claims: Option<i64>,
    /// Explorer-vs-node ledger agreement: "ok" when they match.
    pub ledger_check: Option<String>,
    #[ts(type = "number")]
    pub ledger_check_height: Option<i64>,
    pub peer: Option<bool>,
    #[ts(type = "string")]
    pub last_block_at: Option<DateTimeWithTimeZone>,
    #[ts(type = "string")]
    pub stats_updated_at: Option<DateTimeWithTimeZone>,
    /// When the companion recorded this poll.
    #[ts(type = "string")]
    pub fetched_at: DateTimeWithTimeZone,
}

impl From<chain_snapshots::Model> for SnapshotDto {
    fn from(m: chain_snapshots::Model) -> Self {
        Self {
            id: m.id,
            ok: m.ok,
            error: m.error,
            network: m.network,
            chain_id: m.chain_id,
            height: m.height,
            tip: m.tip,
            bits: m.bits,
            blocks: m.blocks,
            txs: m.txs,
            miners: m.miners,
            shares_per_min: m.shares_per_min,
            escrow: m.escrow,
            sci_paid: m.sci_paid,
            sci_claims: m.sci_claims,
            ledger_check: m.ledger_check,
            ledger_check_height: m.ledger_check_height,
            peer: m.peer,
            last_block_at: m.last_block_at,
            stats_updated_at: m.stats_updated_at,
            fetched_at: m.created_at,
        }
    }
}

/// One mined block: a prime constellation fixed into the chain.
#[derive(serde::Serialize, serde::Deserialize, TS)]
pub struct BlockDto {
    #[ts(type = "number")]
    pub id: i64,
    /// Full block hash.
    pub block_id: String,
    #[ts(type = "number")]
    pub height: i64,
    #[ts(type = "string")]
    pub block_time: DateTimeWithTimeZone,
    /// Miner payout address (64-hex).
    pub miner: String,
    #[ts(type = "number")]
    pub bits: i64,
    /// Constellation size: 5 = quintuplet, 6 = sextuplet.
    #[ts(type = "number")]
    pub tuple: i64,
    pub certified: bool,
    /// The block's prime, decimal.
    pub p: String,
    #[ts(type = "number")]
    pub txs: i64,
    /// On the main chain (not an orphan).
    pub main_chain: bool,
}

impl From<chain_blocks::Model> for BlockDto {
    fn from(m: chain_blocks::Model) -> Self {
        Self {
            id: m.id,
            block_id: m.block_id,
            height: m.height,
            block_time: m.block_time,
            miner: m.miner,
            bits: m.bits,
            tuple: m.tuple,
            certified: m.certified,
            p: m.p,
            txs: m.txs,
            main_chain: m.main_chain,
        }
    }
}

/// The companion's answer to "is the chain healthy right now?".
#[derive(serde::Serialize, serde::Deserialize, TS)]
pub struct StatusDto {
    /// Latest poll succeeded, its data is fresh, the ledger check says "ok",
    /// and the explorer holds a peer connection to its node.
    pub healthy: bool,
    /// The latest poll, or the explorer's own view inside it, is older than
    /// `stale_after_secs`.
    pub stale: bool,
    /// The explorer's ledger check against its node does not say "ok".
    pub ledger_disagreement: bool,
    #[ts(type = "number")]
    pub stale_after_secs: i64,
    pub latest: Option<SnapshotDto>,
}

impl StatusDto {
    /// Fold the latest snapshot into a health summary. `None` (never polled)
    /// is stale and unhealthy — an empty companion is not a healthy one.
    #[must_use]
    pub fn from_latest(latest: Option<&chain_snapshots::Model>, settings: &Settings) -> Self {
        let Some(l) = latest else {
            return Self {
                healthy: false,
                stale: true,
                ledger_disagreement: false,
                stale_after_secs: settings.stale_after_secs,
                latest: None,
            };
        };

        let now = Utc::now();
        let stale_after = chrono::Duration::seconds(settings.stale_after_secs);
        let poll_stale = now.signed_duration_since(l.created_at) > stale_after;
        // A successful poll with no explorer timestamp counts as stale: the
        // freshness of the explorer's own view is the point of the field.
        let stats_stale = l.ok
            && l.stats_updated_at
                .is_none_or(|t| now.signed_duration_since(t) > stale_after);
        let stale = poll_stale || stats_stale;
        let ledger_disagreement = l.ok && l.ledger_check.as_deref() != Some("ok");
        let healthy = l.ok && !stale && !ledger_disagreement && l.peer == Some(true);

        Self {
            healthy,
            stale,
            ledger_disagreement,
            stale_after_secs: settings.stale_after_secs,
            latest: Some(SnapshotDto::from(l.clone())),
        }
    }
}
