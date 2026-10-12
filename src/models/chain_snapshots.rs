pub use super::_entities::chain_snapshots::{ActiveModel, Entity, Model};

use super::_entities::chain_snapshots::Column;
use crate::data::explorer::ExplorerStats;
use crate::models::chain_alerts;
use loco_rs::prelude::*;
use sea_orm::ActiveValue::Set;
use sea_orm::{entity::prelude::*, AccessMode, IsolationLevel};

pub type ChainSnapshots = Entity;

/// An edge-triggered chain alert, derived when a recorded poll changes state
/// against the previous one. Each transition queues one alert delivery, never
/// one per poll; delivery itself is at-least-once.
#[derive(Debug, PartialEq, Eq)]
pub enum ChainAlert {
    /// The explorer stopped answering polls.
    ExplorerDown { error: String },
    /// The explorer answers polls again after a failure.
    ExplorerRecovered,
    /// The explorer's ledger check against its node no longer says "ok" —
    /// the two disagree about balances.
    LedgerDisagreement { check: String },
    /// The ledger check says "ok" again.
    LedgerRestored,
    /// The explorer lost its live peer connection to its node.
    PeerLost,
    /// The explorer reconnected to its node.
    PeerRestored,
}

impl ChainAlert {
    /// Stable key persisted with a pending delivery.
    #[must_use]
    pub const fn alert_type(&self) -> &'static str {
        match self {
            Self::ExplorerDown { .. } => "explorer_down",
            Self::ExplorerRecovered => "explorer_recovered",
            Self::LedgerDisagreement { .. } => "ledger_disagreement",
            Self::LedgerRestored => "ledger_restored",
            Self::PeerLost => "peer_lost",
            Self::PeerRestored => "peer_restored",
        }
    }

    /// Subject for a persisted alert type.
    #[must_use]
    pub fn subject_for_type(alert_type: &str) -> Option<&'static str> {
        match alert_type {
            "explorer_down" => Some("[catasterism] explorer unreachable"),
            "explorer_recovered" => Some("[catasterism] explorer reachable again"),
            "ledger_disagreement" => Some("[catasterism] ledger disagreement"),
            "ledger_restored" => Some("[catasterism] ledger agreement restored"),
            "peer_lost" => Some("[catasterism] explorer lost its node peer"),
            "peer_restored" => Some("[catasterism] explorer peer restored"),
            _ => None,
        }
    }

    /// One-line summary, used as the mail subject.
    #[must_use]
    pub const fn subject(&self) -> &'static str {
        match self {
            Self::ExplorerDown { .. } => "[catasterism] explorer unreachable",
            Self::ExplorerRecovered => "[catasterism] explorer reachable again",
            Self::LedgerDisagreement { .. } => "[catasterism] ledger disagreement",
            Self::LedgerRestored => "[catasterism] ledger agreement restored",
            Self::PeerLost => "[catasterism] explorer lost its node peer",
            Self::PeerRestored => "[catasterism] explorer peer restored",
        }
    }

    /// What happened, in prose, for the mail body.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::ExplorerDown { error } => format!("Polling the explorer failed: {error}"),
            Self::ExplorerRecovered => {
                "The explorer answered a poll again after at least one failure.".to_string()
            }
            Self::LedgerDisagreement { check } => format!(
                "The explorer's ledger check against its node reports {check:?} instead of \
                 \"ok\". The two disagree about account balances."
            ),
            Self::LedgerRestored => {
                "The explorer's ledger check against its node reports \"ok\" again.".to_string()
            }
            Self::PeerLost => {
                "The explorer no longer holds a live peer connection to its node.".to_string()
            }
            Self::PeerRestored => "The explorer reconnected to its node as a peer.".to_string(),
        }
    }
}

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
    /// The most recent snapshot row, if any — the companion's view of "now".
    ///
    /// # Errors
    /// On database failure.
    pub async fn latest<C>(db: &C) -> ModelResult<Option<Self>>
    where
        C: ConnectionTrait,
    {
        Ok(Entity::find()
            .order_by_desc(Column::Id)
            .limit(1)
            .one(db)
            .await?)
    }
}

impl Model {
    /// The most recent `limit` snapshots, newest first.
    ///
    /// # Errors
    /// On database failure.
    pub async fn recent<C>(db: &C, limit: u64) -> ModelResult<Vec<Self>>
    where
        C: ConnectionTrait,
    {
        Ok(Entity::find()
            .order_by_desc(Column::Id)
            .limit(limit)
            .all(db)
            .await?)
    }
}

// implement your write-oriented logic here
impl Model {
    /// Record a successful poll and derive transition alerts against the
    /// previous row.
    ///
    /// # Errors
    /// On database failure.
    pub async fn record_ok(
        db: &DatabaseConnection,
        stats: &ExplorerStats,
    ) -> ModelResult<(Self, Vec<ChainAlert>)> {
        let txn = db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await?;
        let prev = Self::latest(&txn).await?;
        let item = ActiveModel {
            ok: Set(true),
            network: Set(stats.meta.network.clone()),
            chain_id: Set(stats.meta.chain_id.clone()),
            height: Set(stats.meta.height()),
            tip: Set(stats.meta.tip.clone()),
            bits: Set(stats.meta.bits()),
            blocks: Set(stats.meta.blocks()),
            txs: Set(stats.meta.txs()),
            miners: Set(stats.miners),
            shares_per_min: Set(stats.shares_per_min),
            escrow: Set(stats.meta.escrow()),
            sci_paid: Set(stats.meta.sci_paid()),
            sci_claims: Set(stats.meta.sci_claims()),
            ledger_check: Set(stats.meta.check.clone()),
            ledger_check_height: Set(stats.meta.ledger_check_height()),
            peer: Set(stats.meta.peer()),
            last_block_at: Set(stats.last_block_at()),
            stats_updated_at: Set(stats.meta.updated_at()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        let alerts = Self::alerts_between(prev.as_ref(), &item);
        chain_alerts::Model::enqueue_for_snapshot(&txn, item.id, &alerts).await?;
        txn.commit().await?;
        Ok((item, alerts))
    }

    /// Record a failed poll. The row keeps the timeline continuous, so the
    /// next successful poll can see the outage it is recovering from.
    ///
    /// # Errors
    /// On database failure.
    pub async fn record_failure(
        db: &DatabaseConnection,
        error: &str,
    ) -> ModelResult<(Self, Vec<ChainAlert>)> {
        let txn = db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await?;
        let prev = Self::latest(&txn).await?;
        let item = ActiveModel {
            ok: Set(false),
            error: Set(Some(error.to_string())),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        let alerts = Self::alerts_between(prev.as_ref(), &item);
        chain_alerts::Model::enqueue_for_snapshot(&txn, item.id, &alerts).await?;
        txn.commit().await?;
        Ok((item, alerts))
    }
}

// implement your custom finders, selectors oriented logic here
impl Model {
    fn alerts_between(prev: Option<&Self>, cur: &Self) -> Vec<ChainAlert> {
        let Some(prev) = prev else {
            // First poll ever: only a failure is worth mailing.
            return if cur.ok {
                Vec::new()
            } else {
                vec![ChainAlert::ExplorerDown {
                    error: cur.error.clone().unwrap_or_default(),
                }]
            };
        };

        let mut alerts = Vec::new();
        match (prev.ok, cur.ok) {
            (true, false) => alerts.push(ChainAlert::ExplorerDown {
                error: cur.error.clone().unwrap_or_default(),
            }),
            (false, true) => alerts.push(ChainAlert::ExplorerRecovered),
            _ => {}
        }

        // Ledger and peer transitions are only meaningful between two polls
        // that actually carry data.
        if prev.ok && cur.ok {
            let was_ok = prev.ledger_check.as_deref() == Some("ok");
            let now_ok = cur.ledger_check.as_deref() == Some("ok");
            if was_ok && !now_ok {
                alerts.push(ChainAlert::LedgerDisagreement {
                    check: cur
                        .ledger_check
                        .clone()
                        .unwrap_or_else(|| "missing".to_string()),
                });
            } else if !was_ok && now_ok {
                alerts.push(ChainAlert::LedgerRestored);
            }

            match (prev.peer, cur.peer) {
                (Some(true), Some(false)) => alerts.push(ChainAlert::PeerLost),
                (Some(false), Some(true)) => alerts.push(ChainAlert::PeerRestored),
                _ => {}
            }
        }
        alerts
    }
}
