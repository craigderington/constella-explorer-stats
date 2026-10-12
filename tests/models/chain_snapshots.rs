use loco_rs::testing::prelude::*;
use loco_sample::{
    app::App,
    data::explorer::ExplorerStats,
    models::chain_snapshots::{self, ChainAlert},
};
use serial_test::serial;

fn stats(check: &str, peer: &str, height: i64) -> ExplorerStats {
    serde_json::from_str(&format!(
        r#"{{
            "last_block": "2026-10-03T21:28:35Z",
            "meta": {{
                "bits": "448",
                "blocks": "1390",
                "chain_id": "a8f4562e57e74f9d",
                "check": "{check}",
                "check_height": "108597",
                "escrow": "31500000000",
                "height": "{height}",
                "network": "testnet",
                "peer": "{peer}",
                "sci_claims": "710198",
                "sci_paid": "4833500000000",
                "tip": "9ddb19dc",
                "txs": "6",
                "updated_at": "2026-10-03T21:30:17Z"
            }},
            "miners": 7,
            "shares_per_min": 16.4
        }}"#
    ))
    .unwrap()
}

#[tokio::test]
#[serial]
async fn first_successful_poll_records_typed_snapshot_without_alerts() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (snapshot, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 108_615))
        .await
        .unwrap();

    assert!(alerts.is_empty());
    assert!(snapshot.ok);
    assert_eq!(snapshot.height, Some(108_615));
    assert_eq!(snapshot.bits, Some(448));
    assert_eq!(snapshot.escrow, Some(31_500_000_000));
    assert_eq!(snapshot.sci_paid, Some(4_833_500_000_000));
    assert_eq!(snapshot.sci_claims, Some(710_198));
    assert_eq!(snapshot.peer, Some(true));
    assert_eq!(snapshot.ledger_check.as_deref(), Some("ok"));
    assert_eq!(snapshot.network.as_deref(), Some("testnet"));
    assert!(snapshot.stats_updated_at.is_some());
    assert!(snapshot.last_block_at.is_some());
}

#[tokio::test]
#[serial]
async fn first_ever_poll_failure_alerts() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (snapshot, alerts) = chain_snapshots::Model::record_failure(db, "connection refused")
        .await
        .unwrap();

    assert!(!snapshot.ok);
    assert_eq!(
        alerts,
        vec![ChainAlert::ExplorerDown {
            error: "connection refused".to_string()
        }]
    );
}

#[tokio::test]
#[serial]
async fn outage_alerts_once_and_recovery_alerts_once() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 1))
        .await
        .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) = chain_snapshots::Model::record_failure(db, "timeout")
        .await
        .unwrap();
    assert_eq!(
        alerts,
        vec![ChainAlert::ExplorerDown {
            error: "timeout".to_string()
        }]
    );

    // A second consecutive failure must not mail again.
    let (_, alerts) = chain_snapshots::Model::record_failure(db, "timeout")
        .await
        .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 2))
        .await
        .unwrap();
    assert_eq!(alerts, vec![ChainAlert::ExplorerRecovered]);
}

#[tokio::test]
#[serial]
async fn ledger_disagreement_alerts_on_transition_only() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 1))
        .await
        .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) =
        chain_snapshots::Model::record_ok(db, &stats("mismatch: 2 accounts", "true", 2))
            .await
            .unwrap();
    assert_eq!(
        alerts,
        vec![ChainAlert::LedgerDisagreement {
            check: "mismatch: 2 accounts".to_string()
        }]
    );

    // Still disagreeing: no repeat mail.
    let (_, alerts) =
        chain_snapshots::Model::record_ok(db, &stats("mismatch: 3 accounts", "true", 3))
            .await
            .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 4))
        .await
        .unwrap();
    assert_eq!(alerts, vec![ChainAlert::LedgerRestored]);
}

#[tokio::test]
#[serial]
async fn peer_transitions_alert_once_each() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 1))
        .await
        .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "false", 2))
        .await
        .unwrap();
    assert_eq!(alerts, vec![ChainAlert::PeerLost]);

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "false", 3))
        .await
        .unwrap();
    assert!(alerts.is_empty());

    let (_, alerts) = chain_snapshots::Model::record_ok(db, &stats("ok", "true", 4))
        .await
        .unwrap();
    assert_eq!(alerts, vec![ChainAlert::PeerRestored]);
}
