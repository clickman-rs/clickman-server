use chrono::{Duration, Utc};
use clickman_protocol::Event;
use clickman_storage::{
    ProjectSettings, Storage,
    adapters::{ClickHouseAdapter, ClickHouseConfig},
};
use serde_json::json;
use std::{sync::Arc, time::Instant};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("CLICKMAN_TEST_OWNED").as_deref() != Ok("1") {
        return Err("run through scripts/test-platform --clickhouse".into());
    }
    let count: usize = std::env::var("CLICKMAN_BENCH_EVENTS")
        .unwrap_or_else(|_| "20000".into())
        .parse()?;
    if !(500..=1_000_000).contains(&count) {
        return Err("benchmark events must be 500..1000000".into());
    }
    let postgres = Storage::connect(&std::env::var("TEST_DATABASE_URL")?, 8).await?;
    postgres.migrate().await?;
    let native = ClickHouseAdapter::new(ClickHouseConfig {
        url: std::env::var("TEST_CLICKHOUSE_URL")?,
        database: format!("bench_{}", Uuid::new_v4().simple()),
        user: std::env::var("TEST_CLICKHOUSE_USER")?,
        password: std::env::var("TEST_CLICKHOUSE_PASSWORD")?,
    })?;
    let clickhouse = postgres.clone().with_analytics_adapter(Arc::new(native));
    clickhouse.migrate().await?;
    let mut measurements = vec![];
    let now = Utc::now();
    for storage in [postgres, clickhouse] {
        let project = storage
            .create_project(
                &format!("bench-{}", Uuid::new_v4().simple()),
                "Backend benchmark",
            )
            .await?
            .id;
        storage
            .update_settings(
                project,
                ProjectSettings {
                    rotation_lag_seconds: 0,
                    ..Default::default()
                },
            )
            .await?;
        storage.save_funnel(project,json!({"key":"conversion","name":"Conversion","range":"90d","window":"7d","steps":[{"event":"view"},{"event":"purchase"}]})).await?;
        let ingest = Instant::now();
        for start in (0..count).step_by(500) {
            let events: Vec<_> = (start..(start + 500).min(count))
                .map(|n| Event {
                    message_id: Uuid::new_v4(),
                    external_id: format!("actor-{}", n / 10),
                    event: if n % 10 == 9 { "purchase" } else { "view" }.into(),
                    occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
                        (now - Duration::seconds((count - n) as i64))
                            .timestamp_nanos_opt()
                            .unwrap()
                            .into(),
                    )
                    .unwrap(),
                    properties: json!({"plan":"paid","amount":10,"platform":"web"})
                        .as_object()
                        .unwrap()
                        .clone(),
                    context: json!({"os.name":"Web"}).as_object().unwrap().clone(),
                })
                .collect();
            storage
                .insert_server(project, "benchmark", &events, now)
                .await?;
        }
        let ingest_ms = ingest.elapsed().as_millis();
        let rotation = Instant::now();
        let rotated = storage.rotate(project, now).await?;
        let rotation_ms = rotation.elapsed().as_millis();
        let reports = Instant::now();
        storage.refresh_reports(project, now).await?;
        let reports_ms = reports.elapsed().as_millis();
        assert_eq!(rotated, count as u64);
        measurements.push(json!({"backend":storage.backend_name(),"events":count,"actors":count.div_ceil(10),"ingest_ms":ingest_ms,"rotation_ms":rotation_ms,"reports_ms":reports_ms}));
    }
    println!("{}", serde_json::to_string_pretty(&measurements)?);
    Ok(())
}
