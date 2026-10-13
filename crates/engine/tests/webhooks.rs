use async_trait::async_trait;
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    routing::post,
};
use chrono::{Duration, Utc};
use clickman_analytics::Segment;
use clickman_destinations::{SecretResolver, WebhookSender, credential_prefix, webhook_signature};
use clickman_engine::Engine;
use clickman_protocol::Event;
use clickman_storage::{ProjectSettings, Storage};
use serde_json::{Value, json};
use std::{
    future::IntoFuture,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
const SECRET: &str = "owned-fixture-signing-secret";
struct Secrets;
#[async_trait]
impl SecretResolver for Secrets {
    async fn resolve(&self, p: Uuid, r: &str) -> clickman_destinations::Result<String> {
        clickman_destinations::validate_reference(p, r)?;
        Ok(SECRET.into())
    }
}
#[derive(Clone)]
struct Receiver {
    fail: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<String>>>,
}
async fn receive(State(receiver): State<Receiver>, request: Request) -> StatusCode {
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 65536).await.unwrap();
    let signature = parts.headers["x-clickman-signature"].to_str().unwrap();
    let timestamp = signature
        .strip_prefix("t=")
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(signature, webhook_signature(SECRET, timestamp, &bytes));
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let id = value["id"].as_str().unwrap();
    assert_eq!(parts.headers["x-clickman-event-id"], id);
    receiver.events.lock().unwrap().push(id.into());
    if receiver.fail.load(Ordering::SeqCst) {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::NO_CONTENT
    }
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL"]
async fn signed_webhook_failure_restart_replay_and_network_policy_are_real() {
    assert_eq!(std::env::var("CLICKMAN_TEST_OWNED").unwrap(), "1");
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 8)
        .await
        .unwrap();
    storage.migrate().await.unwrap();
    let id = storage
        .create_project(&format!("hooks-{}", Uuid::new_v4()), "Hooks")
        .await
        .unwrap()
        .id;
    storage
        .update_settings(
            id,
            ProjectSettings {
                rotation_lag_seconds: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let definition = json!({"key":"viewers","name":"Viewers","rule":{"kind":"event","filter":{"event":"view"},"range":"7d"}});
    assert!(Segment::parse(definition.clone(), &[]).is_ok());
    let segment = storage
        .save_segment(id, definition, true, true)
        .await
        .unwrap();
    let receiver = Receiver {
        fail: Arc::new(AtomicBool::new(true)),
        events: Arc::new(Mutex::new(vec![])),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/catch", listener.local_addr().unwrap());
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/catch", post(receive))
                .with_state(receiver.clone()),
        )
        .into_future(),
    );
    let hook = storage
        .save_webhook(
            id,
            segment.id,
            "Receiver",
            "webhook",
            json!({"url":url,"signing_ref":format!("{}SIGNING",credential_prefix(id))}),
            true,
        )
        .await
        .unwrap();
    let now = Utc::now();
    storage
        .insert_server(
            id,
            "web",
            &[Event {
                message_id: Uuid::new_v4(),
                event: "view".into(),
                external_id: "customer".into(),
                occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
                    now.timestamp_nanos_opt().unwrap().into(),
                )
                .unwrap(),
                properties: Default::default(),
                context: Default::default(),
            }],
            now,
        )
        .await
        .unwrap();
    storage.rotate(id, now).await.unwrap();
    assert_eq!(storage.evaluate_segments(id, now).await.unwrap(), 1);
    let sender = WebhookSender::default().with_secrets(Arc::new(Secrets));
    let delivery = storage.webhook_delivery(id, now).await.unwrap().unwrap();
    assert!(sender.deliver(&hook, &delivery.payload).await.is_err());
    drop(delivery);
    assert!(receiver.events.lock().unwrap().is_empty());
    let make = || {
        Engine::new(storage.clone())
            .unwrap()
            .with_webhook_sender(sender.clone().allow_private_networks(true))
    };
    let engine = make();
    assert_eq!(engine.jobs().deliver_webhooks(id, now).await.unwrap(), 0);
    let failed = storage.webhook_logs(id, 100).await.unwrap();
    assert_eq!(failed[0].status, "pending");
    assert_eq!(failed[0].last_status, Some(503));
    assert_eq!(failed[0].attempts, 1);
    drop(engine);
    receiver.fail.store(false, Ordering::SeqCst);
    let restarted = make();
    assert_eq!(
        restarted
            .jobs()
            .deliver_webhooks(id, now + Duration::seconds(10))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        storage.webhook_logs(id, 100).await.unwrap()[0].status,
        "delivered"
    );
    assert!(storage.replay_webhook(id, failed[0].id).await.unwrap());
    assert_eq!(
        restarted
            .jobs()
            .deliver_webhooks(id, now + Duration::seconds(10))
            .await
            .unwrap(),
        1
    );
    let events = receiver.events.lock().unwrap().clone();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event == &events[0]));
    // A success before a crash/ack loss is replayed with the same stable ID.
    storage.replay_webhook(id, failed[0].id).await.unwrap();
    let delivery = storage
        .webhook_delivery(id, now + Duration::seconds(10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        sender
            .clone()
            .allow_private_networks(true)
            .deliver(&delivery.webhook, &delivery.payload)
            .await
            .unwrap(),
        204
    );
    drop(delivery);
    assert_eq!(
        restarted
            .jobs()
            .deliver_webhooks(id, now + Duration::seconds(10))
            .await
            .unwrap(),
        1
    );
    assert!(storage.erase(id, "customer", now).await.unwrap());
    assert!(storage.webhook_logs(id, 100).await.unwrap().is_empty());
    assert!(storage.webhook_delivery(id, now).await.unwrap().is_none());
    server.abort();
    let _ = server.await;
}
