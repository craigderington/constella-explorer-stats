use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use loco_rs::{boot::run_task, task, testing::prelude::*};
use loco_sample::{
    app::App,
    models::{chain_alerts, chain_snapshots},
};
use serde::Deserialize;
use serial_test::serial;
use std::sync::{Arc, Mutex};
use tokio::{net::TcpListener, task::JoinHandle};

#[derive(Clone)]
struct MockMailgun {
    messages: Arc<Mutex<Vec<CapturedMessage>>>,
    response_status: StatusCode,
}

#[derive(Clone, Debug, Deserialize)]
struct MailgunForm {
    from: String,
    to: String,
    subject: String,
    text: String,
}

#[derive(Clone, Debug)]
struct CapturedMessage {
    authorization: Option<String>,
    form: MailgunForm,
}

async fn capture_mailgun(
    State(mock): State<MockMailgun>,
    headers: HeaderMap,
    Form(form): Form<MailgunForm>,
) -> StatusCode {
    mock.messages.lock().unwrap().push(CapturedMessage {
        authorization: headers
            .get("authorization")
            .and_then(|header| header.to_str().ok())
            .map(str::to_owned),
        form,
    });
    mock.response_status
}

async fn start_mock_mailgun(
    response_status: StatusCode,
) -> (String, Arc<Mutex<Vec<CapturedMessage>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let messages = Arc::new(Mutex::new(Vec::new()));
    let mock = MockMailgun {
        messages: Arc::clone(&messages),
        response_status,
    };
    let app = Router::new()
        .route("/v3/{domain}/messages", post(capture_mailgun))
        .with_state(mock);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{address}"), messages, server)
}

fn set_mailgun_base_url(boot: &mut loco_rs::boot::BootResult, base_url: String) {
    let settings = boot
        .app_context
        .config
        .settings
        .as_mut()
        .expect("test settings are configured");
    settings["mailgun"]["api_base_url"] = serde_json::Value::String(base_url);
}

/// The test config points the companion at a closed port, so the poll fails.
/// That failure is the data under test: the task must record it, alert on it
/// (first poll ever → `ExplorerDown`), and still exit successfully so the
/// scheduler keeps polling.
#[tokio::test]
#[serial]
async fn unreachable_explorer_is_recorded_and_emailed_through_mailgun() {
    let (base_url, messages, server) = start_mock_mailgun(StatusCode::OK).await;
    let mut boot = boot_test::<App>().await.unwrap();
    set_mailgun_base_url(&mut boot, base_url);

    assert!(run_task::<App>(
        &boot.app_context,
        Some(&"chain_poll".to_string()),
        &task::Vars::default()
    )
    .await
    .is_ok());

    let latest = chain_snapshots::Model::latest(&boot.app_context.db)
        .await
        .unwrap()
        .expect("a poll records a row even when it fails");
    assert!(!latest.ok);
    assert!(latest.error.unwrap().contains("api/stats"));

    assert!(chain_alerts::Model::pending(&boot.app_context.db)
        .await
        .unwrap()
        .is_empty());
    server.abort();

    let messages = messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0]
        .authorization
        .as_deref()
        .is_some_and(|header| header.starts_with("Basic ")));
    assert_eq!(
        messages[0].form.from,
        "Catasterism Alerts <alerts@example.test>"
    );
    assert_eq!(messages[0].form.to, "alerts@example.com");
    assert_eq!(
        messages[0].form.subject,
        "[catasterism] explorer unreachable"
    );
    assert!(messages[0]
        .form
        .text
        .contains("Polling the explorer failed"));
}

#[tokio::test]
#[serial]
async fn mailgun_rejection_leaves_alert_pending_for_retry() {
    let (base_url, _messages, server) = start_mock_mailgun(StatusCode::UNAUTHORIZED).await;
    let mut boot = boot_test::<App>().await.unwrap();
    set_mailgun_base_url(&mut boot, base_url);

    assert!(run_task::<App>(
        &boot.app_context,
        Some(&"chain_poll".to_string()),
        &task::Vars::default()
    )
    .await
    .is_err());

    assert_eq!(
        chain_alerts::Model::pending(&boot.app_context.db)
            .await
            .unwrap()
            .len(),
        1
    );
    server.abort();
}
