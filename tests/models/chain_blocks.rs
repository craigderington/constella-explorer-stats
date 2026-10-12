use loco_rs::testing::prelude::*;
use loco_sample::{app::App, data::explorer::ExplorerBlock, models::chain_blocks};
use sea_orm::EntityTrait;
use serial_test::serial;

const BLOCKS: &str = r#"[
    {
        "id": "99b66988f45c42ea5b1994e9e76de3a74ba9512e61dc3d846d28f3daf78cff1b",
        "height": 108592,
        "time": "2026-10-03T21:28:35Z",
        "miner": "34d89b9ee084c78196b0ae1709dc08276a8ef90068149c1a7db451aa0280d84d",
        "bits": 448,
        "tuple": 5,
        "block": true,
        "certified": true,
        "p": "387251822093895454342874956580647339240251834954973327774977273789624977902314748030106247582092912584684878766099584021443805043905397",
        "txs": 0,
        "main": true
    },
    {
        "id": "be47b40b3a43214201660400779154460013aec36376e44cd2fc3dab8d773c23",
        "height": 108588,
        "time": "2026-10-03T21:28:21Z",
        "miner": "3ec619ac279656341b7782b6e445238104e593e87c2fc4511e3bfe948102da49",
        "bits": 448,
        "tuple": 5,
        "block": true,
        "certified": true,
        "p": "543751084407186149874395202954546222937710652248784968423852693370864765714054223174308401077641714780409359482884433917136951255962617",
        "txs": 0,
        "main": true
    }
]"#;

const THIRD_BLOCK: &str = r#"{
    "id": "86834121c92ee3c533d9f6f6d08c5c03a9a2fd682451c87594da4304b375feb8",
    "height": 108419,
    "time": "2026-10-03T21:19:03Z",
    "miner": "3ec619ac279656341b7782b6e445238104e593e87c2fc4511e3bfe948102da49",
    "bits": 432,
    "tuple": 5,
    "block": true,
    "certified": true,
    "p": "782566717734736682562542329275247671370968583004002335278448128124553164496871627604776849599581704553164496871627604776457",
    "txs": 0,
    "main": true
}"#;

#[tokio::test]
#[serial]
async fn insert_new_dedups_by_block_hash() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let mut blocks: Vec<ExplorerBlock> = serde_json::from_str(BLOCKS).unwrap();
    blocks.extend(serde_json::from_str::<Vec<ExplorerBlock>>(BLOCKS).unwrap());

    assert_eq!(
        chain_blocks::Model::insert_new(db, &blocks).await.unwrap(),
        2
    );
    // Same batch again: nothing new.
    assert_eq!(
        chain_blocks::Model::insert_new(db, &blocks).await.unwrap(),
        0
    );

    // Overlapping batch: only the unseen hash lands.
    let mut mixed = blocks;
    mixed.push(serde_json::from_str(THIRD_BLOCK).unwrap());
    assert_eq!(
        chain_blocks::Model::insert_new(db, &mixed).await.unwrap(),
        1
    );

    let all = chain_blocks::Entity::find().all(db).await.unwrap();
    assert_eq!(all.len(), 3);
    let stored = all.iter().find(|b| b.height == 108_592).unwrap();
    assert!(stored.main_chain);
    assert!(stored.certified);
    assert_eq!(stored.bits, 448);
    assert_eq!(stored.tuple, 5);
    assert_eq!(stored.txs, 0);
}

#[tokio::test]
#[serial]
async fn empty_batch_inserts_nothing() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    assert_eq!(chain_blocks::Model::insert_new(db, &[]).await.unwrap(), 0);
}

#[tokio::test]
#[serial]
async fn block_with_unparseable_time_is_skipped_not_guessed() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let mut blocks: Vec<ExplorerBlock> = serde_json::from_str(BLOCKS).unwrap();
    let mut broken: ExplorerBlock = serde_json::from_str(THIRD_BLOCK).unwrap();
    broken.time = "not-a-timestamp".to_string();
    blocks.push(broken);

    assert_eq!(
        chain_blocks::Model::insert_new(db, &blocks).await.unwrap(),
        2
    );
    assert_eq!(chain_blocks::Entity::find().all(db).await.unwrap().len(), 2);
}
