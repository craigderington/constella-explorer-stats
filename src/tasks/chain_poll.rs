use std::time::Duration;

use loco_rs::prelude::*;

use crate::{
    data::explorer::{ExplorerBlock, ExplorerStats, Settings},
    mailers::chain_alert::ChainAlertMailer,
    models::{chain_alerts, chain_blocks, chain_snapshots},
};

pub struct ChainPoll;

/// Fetch one JSON endpoint and decode it, turning every failure into a
/// one-line message fit for the snapshot row and an alert mail.
async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> std::result::Result<T, String> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GET {url}: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("decode {url}: {e}"))
}

#[async_trait]
impl Task for ChainPoll {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "chain_poll".to_string(),
            detail:
                "Poll the Catasterism explorer, record a chain snapshot, mail transition alerts"
                    .to_string(),
        }
    }

    /// A failed explorer fetch is data, not a task error: it is recorded and
    /// the scheduler continues. Mail errors fail the run while leaving the
    /// outbox entry pending for the next poll.
    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        let settings = Settings::from_context(ctx)?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("loco-sample-companion/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::string(&format!("could not build http client: {e}")))?;

        let stats = fetch_json::<ExplorerStats>(&client, &settings.stats_url()).await;

        let (snapshot, _alerts) = match stats {
            Ok(stats) => {
                let (snapshot, alerts) = chain_snapshots::Model::record_ok(&ctx.db, &stats).await?;
                // Blocks are enrichment: failing to fetch them must not void
                // the stats snapshot that already recorded.
                match fetch_json::<Vec<ExplorerBlock>>(&client, &settings.blocks_url()).await {
                    Ok(blocks) => {
                        let inserted = chain_blocks::Model::insert_new(&ctx.db, &blocks).await?;
                        tracing::info!(inserted, "stored new blocks");
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "block fetch failed; snapshot recorded anyway");
                    }
                }
                (snapshot, alerts)
            }
            Err(err) => chain_snapshots::Model::record_failure(&ctx.db, &err).await?,
        };

        for delivery in chain_alerts::Model::pending(&ctx.db).await? {
            let subject = chain_snapshots::ChainAlert::subject_for_type(&delivery.alert_type)
                .ok_or_else(|| Error::string("unknown persisted chain alert type"))?;
            let snapshot = chain_snapshots::Entity::find_by_id(delivery.chain_snapshots)
                .one(&ctx.db)
                .await?
                .ok_or_else(|| Error::string("chain alert references a missing snapshot"))?;
            ChainAlertMailer::send_alert(
                ctx,
                &settings.alert_email,
                subject,
                &delivery.detail,
                &snapshot,
            )
            .await?;
            delivery.mark_delivered(&ctx.db).await?;
        }

        let alerts_pending = chain_alerts::Model::pending(&ctx.db).await?.len();
        tracing::info!(
            ok = snapshot.ok,
            height = snapshot.height,
            alerts_pending,
            "chain poll recorded"
        );
        Ok(())
    }
}
