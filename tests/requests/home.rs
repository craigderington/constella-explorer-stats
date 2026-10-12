use loco_rs::testing::prelude::*;
use loco_sample::{
    app::App,
    models::{chain_blocks, chain_snapshots},
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn index_renders_an_empty_state_before_any_poll() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/").await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains("No data yet"));
        assert!(html.contains("No blocks stored yet"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn index_shows_the_latest_poll_and_escapes_untrusted_text() {
    request::<App, _, _>(|request, ctx| async move {
        chain_snapshots::Model::record_failure(&ctx.db, "<script>alert(1)</script>")
            .await
            .unwrap();
        chain_blocks::Model::insert_new(&ctx.db, &[]).await.unwrap();

        let res = request.get("/").await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains("Chain degraded"));
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&lt;script&gt;"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn index_renders_blocks_and_a_healthy_poll() {
    request::<App, _, _>(|request, ctx| async move {
        let now = chrono::Utc::now().to_rfc3339();
        let stats: loco_sample::data::explorer::ExplorerStats = serde_json::from_str(&format!(
            r#"{{"last_block":"{now}","meta":{{"check":"ok","height":"144762","peer":"true",
            "network":"testnet","bits":"456","escrow":"31500000000","updated_at":"{now}"}},
            "miners":7,"shares_per_min":16.4}}"#
        ))
        .unwrap();
        chain_snapshots::Model::record_ok(&ctx.db, &stats)
            .await
            .unwrap();
        let blocks: Vec<loco_sample::data::explorer::ExplorerBlock> = serde_json::from_str(
            r#"[{"id":"99b66988f45c42ea5b1994e9e76de3a74ba9512e61dc3d846d28f3daf78cff1b",
            "height":108592,"time":"2026-10-03T21:28:35Z","miner":"34d89b9ee084c78196b0ae1709dc08276a8ef90068149c1a7db451aa0280d84d",
            "bits":448,"tuple":5,"block":true,"certified":true,"p":"7","txs":0,"main":true}]"#,
        )
        .unwrap();
        chain_blocks::Model::insert_new(&ctx.db, &blocks)
            .await
            .unwrap();

        let res = request.get("/").await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains("Chain healthy"));
        assert!(html.contains("144,762"));
        assert!(html.contains("315.00"));
        assert!(html.contains("108,592"));
        assert!(html.contains("quintuplet"));
    })
    .await;
}
