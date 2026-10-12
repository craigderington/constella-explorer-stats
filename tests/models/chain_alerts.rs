use loco_rs::testing::prelude::*;
use loco_sample::{app::App, models::chain_alerts, models::chain_snapshots};
use serial_test::serial;

macro_rules! configure_insta {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        let _guard = settings.bind_to_scope();
    };
}

/// The `ChainAlerts` entity matches the table its migration created.
///
/// Selecting every column is the cheapest assertion that says something true:
/// it fails if the migration and the entity disagree — a renamed column, a
/// type that does not round-trip, a migration that never ran — which is the
/// most common way a generated model breaks. Extend it as the model grows.
#[tokio::test]
#[serial]
async fn transition_alerts_remain_pending_until_mailed() {
    configure_insta!();

    let boot = boot_test::<App>().await.unwrap();
    let (snapshot, alerts) =
        chain_snapshots::Model::record_failure(&boot.app_context.db, "connection refused")
            .await
            .unwrap();

    assert_eq!(alerts.len(), 1);
    let pending = chain_alerts::Model::pending(&boot.app_context.db)
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].alert_type, "explorer_down");
    assert_eq!(pending[0].chain_snapshots, snapshot.id);
    assert_eq!(
        chain_snapshots::ChainAlert::subject_for_type(&pending[0].alert_type),
        Some("[catasterism] explorer unreachable")
    );

    pending
        .into_iter()
        .next()
        .unwrap()
        .mark_delivered(&boot.app_context.db)
        .await
        .unwrap();
    assert!(chain_alerts::Model::pending(&boot.app_context.db)
        .await
        .unwrap()
        .is_empty());
}
