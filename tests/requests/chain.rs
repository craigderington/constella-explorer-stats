use loco_rs::testing::prelude::*;
use loco_sample::{
    app::App,
    data::explorer::{ExplorerBlock, ExplorerStats},
    models::{chain_blocks, chain_snapshots},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use serial_test::serial;

fn fresh_stats(check: &str, peer: &str, height: i64) -> ExplorerStats {
    let now = chrono::Utc::now().to_rfc3339();
    serde_json::from_str(&format!(
        r#"{{
            "last_block": "{now}",
            "meta": {{
                "bits": "448",
                "blocks": "1390",
                "chain_id": "a8f4562e57e74f9d",
                "check": "{check}",
                "check_height": "{height}",
                "escrow": "31500000000",
                "height": "{height}",
                "network": "testnet",
                "peer": "{peer}",
                "sci_claims": "710198",
                "sci_paid": "4833500000000",
                "tip": "9ddb19dc",
                "txs": "6",
                "updated_at": "{now}"
            }},
            "miners": 7,
            "shares_per_min": 16.4
        }}"#
    ))
    .unwrap()
}

const BLOCKS: &str = r#"[
    {
        "id": "99b66988f45c42ea5b1994e9e76de3a74ba9512e61dc3d846d28f3daf78cff1b",
        "height": 108592,
        "time": "2026-10-03T21:28:35Z",
        "miner": "34d89b9ee084c78196b0ae1709dc08276a8ef90068149c1a7db451aa0280d84d",
        "bits": 448, "tuple": 5, "block": true, "certified": true,
        "p": "387251822093895454342874956580647339240251834954973327774977273789624977902314748030106247582092912584684878766099584021443805043905397",
        "txs": 0, "main": true
    },
    {
        "id": "be47b40b3a43214201660400779154460013aec36376e44cd2fc3dab8d773c23",
        "height": 108588,
        "time": "2026-10-03T21:28:21Z",
        "miner": "3ec619ac279656341b7782b6e445238104e593e87c2fc4511e3bfe948102da49",
        "bits": 448, "tuple": 5, "block": true, "certified": true,
        "p": "543751084407186149874395202954546222937710652248784968423852693370864765714054223174308401077641714780409359482884433917136951255962617",
        "txs": 0, "main": true
    }
]"#;

#[tokio::test]
#[serial]
async fn status_is_unhealthy_before_any_poll() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/chain/status").await;
        assert_eq!(res.status_code(), 200);

        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["stale"], true);
        assert_eq!(body["ledger_disagreement"], false);
        assert!(body["latest"].is_null());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_reports_healthy_chain_after_fresh_poll() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_ok(&ctx.db, &fresh_stats("ok", "true", 108_615))
            .await
            .unwrap();

        let res = request.get("/api/chain/status").await;
        assert_eq!(res.status_code(), 200);

        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], true);
        assert_eq!(body["stale"], false);
        assert_eq!(body["ledger_disagreement"], false);
        assert_eq!(body["latest"]["height"], 108_615);
        assert_eq!(body["latest"]["bits"], 448);
        assert_eq!(body["latest"]["escrow"], 31_500_000_000_i64);
        assert_eq!(body["latest"]["peer"], true);
        assert_eq!(body["latest"]["ledger_check"], "ok");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_marks_a_poll_stale_after_the_exact_threshold() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_ok(&ctx.db, &fresh_stats("ok", "true", 108_615))
            .await
            .unwrap();
        let latest = chain_snapshots::Model::latest(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut active = latest.into_active_model();
        active.created_at =
            Set((chrono::Utc::now() - chrono::Duration::milliseconds(300_500)).into());
        active.update(&ctx.db).await.unwrap();

        let res = request.get("/api/chain/status").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["stale"], true);
        assert_eq!(body["ledger_disagreement"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_marks_missing_explorer_timestamp_stale() {
    request::<App, _, _>(|request, ctx| async move {
        let stats: ExplorerStats =
            serde_json::from_str(r#"{"meta":{"check":"ok","peer":"true"}}"#).unwrap();
        chain_snapshots::Model::record_ok(&ctx.db, &stats)
            .await
            .unwrap();

        let res = request.get("/api/chain/status").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["stale"], true);
        assert_eq!(body["ledger_disagreement"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn recent_failed_poll_is_unhealthy_but_not_stale() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_failure(&ctx.db, "connection refused")
            .await
            .unwrap();

        let res = request.get("/api/chain/status").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["stale"], false);
        assert_eq!(body["ledger_disagreement"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_flags_ledger_disagreement() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_ok(
            &ctx.db,
            &fresh_stats("mismatch: 2 accounts", "true", 108_615),
        )
        .await
        .unwrap();

        let res = request.get("/api/chain/status").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["ledger_disagreement"], true);
        assert_eq!(body["latest"]["ledger_check"], "mismatch: 2 accounts");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_flags_lost_peer() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_ok(&ctx.db, &fresh_stats("ok", "false", 108_615))
            .await
            .unwrap();

        let res = request.get("/api/chain/status").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["healthy"], false);
        assert_eq!(body["ledger_disagreement"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn snapshots_paginate_newest_first() {
    request::<App, _, _>(|request, ctx| async move {
        for height in [1, 2, 3] {
            chain_snapshots::Model::record_ok(&ctx.db, &fresh_stats("ok", "true", height))
                .await
                .unwrap();
        }

        let res = request.get("/api/chain/snapshots?page=1&page_size=2").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["items"].as_array().unwrap().len(), 2);
        assert_eq!(body["total_items"], 3);
        assert_eq!(body["total_pages"], 2);
        assert_eq!(body["page"], 1);
        assert_eq!(body["items"][0]["height"], 3);
        assert_eq!(body["items"][1]["height"], 2);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn blocks_list_newest_first() {
    request::<App, _, _>(|request, ctx| async move {
        let blocks: Vec<ExplorerBlock> = serde_json::from_str(BLOCKS).unwrap();
        chain_blocks::Model::insert_new(&ctx.db, &blocks)
            .await
            .unwrap();

        let res = request.get("/api/chain/blocks").await;
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["height"], 108_592);
        assert_eq!(
            items[0]["block_id"],
            "99b66988f45c42ea5b1994e9e76de3a74ba9512e61dc3d846d28f3daf78cff1b"
        );
        assert_eq!(items[0]["main_chain"], true);
        assert_eq!(items[1]["height"], 108_588);
    })
    .await;
}
