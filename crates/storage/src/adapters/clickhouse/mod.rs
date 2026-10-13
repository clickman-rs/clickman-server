mod reports;

use super::{AnalyticsAdapter, PostgresAdapter};
use crate::{Error, MetadataStore, ProjectSettings, Result, WriteIdentity, lock_project, sql};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, Utc};
use clickhouse::{Client, Row};
use clickman_protocol::Event;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{future::Future, sync::Arc};
use uuid::Uuid;

/// Credentials are operator configuration, never project payloads or Debug output.
pub struct ClickHouseConfig {
    pub url: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

#[derive(Clone)]
pub struct ClickHouseAdapter {
    client: Client,
    database: String,
}

#[derive(Row, Serialize, Deserialize)]
struct NativeEvent {
    #[serde(with = "clickhouse::serde::uuid")]
    project_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    message_id: Uuid,
    external_id: String,
    source: String,
    event: String,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    occurred_at: DateTime<Utc>,
    #[serde(with = "clickhouse::serde::chrono::datetime64::micros")]
    received_at: DateTime<Utc>,
    properties: String,
    context: String,
}

impl ClickHouseAdapter {
    pub fn new(config: ClickHouseConfig) -> Result<Self> {
        let endpoint = url::Url::parse(&config.url)
            .map_err(|_| Error::Invalid("invalid ClickHouse URL".into()))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(Error::Invalid(
                "ClickHouse URL must be HTTP(S), without embedded credentials, query or fragment"
                    .into(),
            ));
        }
        if config.database.is_empty()
            || config.database.len() > 64
            || !config
                .database
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(Error::Invalid("invalid ClickHouse database name".into()));
        }
        let client = Client::default()
            .with_url(config.url)
            .with_database(&config.database)
            .with_user(config.user)
            .with_password(config.password)
            .with_setting("max_execution_time", "20");
        Ok(Self {
            client,
            database: config.database,
        })
    }
    fn independent(metadata: &MetadataStore) -> Result<()> {
        if metadata.transaction_scoped() {
            return Err(Error::Invalid("remote analytics operations require an independently committed store; use PostgreSQL for caller-transaction atomicity".into()));
        }
        Ok(())
    }
    async fn bounded<T>(
        operation: &'static str,
        future: impl Future<Output = clickhouse::error::Result<T>>,
    ) -> Result<T> {
        match tokio::time::timeout(std::time::Duration::from_secs(25), future).await {
            Ok(result) => result.map_err(|source| Error::Backend {
                operation,
                source: Box::new(source),
            }),
            Err(source) => Err(Error::Backend {
                operation,
                source: Box::new(source),
            }),
        }
    }
}

#[async_trait]
impl AnalyticsAdapter for ClickHouseAdapter {
    fn name(&self) -> &'static str {
        "clickhouse"
    }
    async fn observations(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        query: &crate::ObservationQuery,
    ) -> Result<Vec<clickman_analytics::Occurrence>> {
        let db = metadata.connection();
        let actors:std::collections::BTreeMap<String,i64>=db.query_all_raw(sql("SELECT a.external_id,c.id FROM clickman_actors a JOIN clickman_actors c ON c.project_id=a.project_id AND c.id=COALESCE(a.merged_into_id,a.id) WHERE a.project_id=$1 AND a.erased_at IS NULL AND c.erased_at IS NULL AND c.external_id<>'*'",vec![project.into()])).await?.into_iter().map(|r|Ok((r.try_get("","external_id")?,r.try_get("","id")?))).collect::<Result<_>>()?;
        let request=self.client.query("SELECT ?fields FROM events FINAL WHERE project_id=? AND external_id<>'*' AND event IN ? AND occurred_at BETWEEN fromUnixTimestamp64Milli(?) AND fromUnixTimestamp64Milli(?) ORDER BY occurred_at,message_id,received_at").bind(project).bind(&query.events).bind(query.from.timestamp_millis()).bind(query.to.timestamp_millis());
        let mut cursor = request
            .fetch::<NativeEvent>()
            .map_err(|source| Error::Backend {
                operation: "segment observations",
                source: Box::new(source),
            })?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
        let mut observations = Vec::new();
        loop {
            let row = tokio::time::timeout_at(deadline, cursor.next())
                .await
                .map_err(|source| Error::Backend {
                    operation: "segment observations",
                    source: Box::new(source),
                })?
                .map_err(|source| Error::Backend {
                    operation: "segment observations",
                    source: Box::new(source),
                })?;
            let Some(row) = row else { break };
            let actor = *actors.get(&row.external_id).ok_or_else(|| {
                Error::Invalid("analytics export requires a completed metadata rotation".into())
            })?;
            let properties =
                serde_json::from_str(&row.properties).map_err(|e| Error::Invalid(e.to_string()))?;
            let context =
                serde_json::from_str(&row.context).map_err(|e| Error::Invalid(e.to_string()))?;
            observations.push(clickman_analytics::Occurrence {
                actor,
                at: row.occurred_at.timestamp_millis(),
                event: row.event,
                fields: crate::observations::select_fields(properties, context, &query.fields),
            });
        }
        Ok(observations)
    }
    async fn migrate(&self, metadata: &MetadataStore) -> Result<()> {
        Self::independent(metadata)?;
        let default = self.client.clone().with_database("default");
        Self::bounded(
            "create database",
            default
                .query(&format!(
                    "CREATE DATABASE IF NOT EXISTS `{}`",
                    self.database
                ))
                .execute(),
        )
        .await?;
        Self::bounded("create events", self.client.query("CREATE TABLE IF NOT EXISTS events (project_id UUID, message_id UUID, external_id String, source LowCardinality(String), event LowCardinality(String), occurred_at DateTime64(3,'UTC'), received_at DateTime64(6,'UTC'), properties String, context String) ENGINE=ReplacingMergeTree PARTITION BY toYYYYMM(occurred_at) ORDER BY (project_id,message_id,received_at)").execute()).await
    }
    async fn healthy(&self, _: &MetadataStore) -> Result<()> {
        Self::bounded("health", self.client.query("SELECT 1").execute()).await
    }
    async fn insert(
        &self,
        metadata: &MetadataStore,
        identity: &WriteIdentity,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        // The PostgreSQL inbox is the durable acceptance/dedup/delivery boundary.
        PostgresAdapter
            .insert(metadata, identity, events, received_at)
            .await
    }
    async fn insert_server(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        source: &str,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        PostgresAdapter
            .insert_server(metadata, project, source, events, received_at)
            .await
    }
    async fn rotate(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        Self::independent(metadata)?;
        let mut processed = 0;
        loop {
            let tx = metadata.db.begin().await?;
            let p = lock_project(&tx, project).await?;
            let settings: ProjectSettings =
                serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
            let raw = tx.query_all_raw(sql("SELECT message_id,external_id,source,event,occurred_at,received_at,properties,context FROM clickman_events WHERE project_id=$1 AND rotated_at IS NULL AND received_at<=$2 ORDER BY received_at,message_id LIMIT 2000", vec![project.into(), (now-Duration::seconds(settings.rotation_lag_seconds.into())).into()])).await?;
            if raw.is_empty() {
                tx.commit().await?;
                break;
            }
            let rows: Vec<NativeEvent> = raw
                .into_iter()
                .map(|r| {
                    Ok(NativeEvent {
                        project_id: project,
                        message_id: r.try_get("", "message_id")?,
                        external_id: r.try_get("", "external_id")?,
                        source: r.try_get("", "source")?,
                        event: r.try_get("", "event")?,
                        occurred_at: r.try_get("", "occurred_at")?,
                        received_at: r.try_get("", "received_at")?,
                        properties: r.try_get::<Value>("", "properties")?.to_string(),
                        context: r.try_get::<Value>("", "context")?.to_string(),
                    })
                })
                .collect::<Result<_>>()?;
            let first: Vec<_> = rows.iter().map(|r| json!({"external_id":r.external_id,"first_seen_on":r.occurred_at.date_naive()})).collect();
            tx.execute_raw(sql("INSERT INTO clickman_actors(project_id,external_id,first_seen_on) SELECT $1,external_id,min(first_seen_on) FROM jsonb_to_recordset($2) AS a(external_id text,first_seen_on date) GROUP BY external_id ON CONFLICT(project_id,external_id) DO UPDATE SET first_seen_on=CASE WHEN clickman_actors.has_events THEN LEAST(clickman_actors.first_seen_on,EXCLUDED.first_seen_on) ELSE EXCLUDED.first_seen_on END,has_events=true", vec![project.into(),json!(first).into()])).await?;
            crate::identity::propagate_cohorts(&tx, project).await?;
            Self::bounded("export events", async {
                let mut insert = self.client.insert::<NativeEvent>("events").await?;
                for row in &rows {
                    insert.write(row).await?;
                }
                insert.end().await
            })
            .await?;
            let ids: Vec<_> = rows.iter().map(|r| r.message_id).collect();
            tx.execute_raw(sql("UPDATE clickman_events SET rotated_at=$3 WHERE project_id=$1 AND message_id IN (SELECT value::uuid FROM jsonb_array_elements_text($2))",vec![project.into(),json!(ids).into(),now.into()])).await?;
            tx.commit().await?;
            processed += rows.len() as u64;
            if rows.len() < 2000 {
                break;
            }
        }
        Ok(processed)
    }
    async fn prune(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        Self::independent(metadata)?;
        let tx = metadata.db.begin().await?;
        let p = lock_project(&tx, project).await?;
        let settings: ProjectSettings =
            serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
        if let Some(days) = settings.retention_days {
            let boundary = (now - Duration::days(days.into()))
                .date_naive()
                .with_day(1)
                .expect("valid month");
            Self::bounded("retention", self.client.query("ALTER TABLE events DELETE WHERE project_id=? AND toDate(occurred_at) < toDate(?) SETTINGS mutations_sync=2").bind(project).bind(boundary.to_string()).execute()).await?;
        }
        let count = tx.execute_raw(sql("DELETE FROM clickman_events e WHERE e.project_id=$1 AND e.rotated_at IS NOT NULL AND e.received_at<$2 AND NOT EXISTS(SELECT 1 FROM clickman_deliveries d WHERE d.project_id=e.project_id AND d.message_id=e.message_id AND d.completed_at IS NULL)",vec![project.into(),(now-Duration::days(settings.dedup_window_days.into())).into()])).await?.rows_affected();
        tx.commit().await?;
        Ok(count)
    }
    async fn erase(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        external: &str,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        Self::independent(metadata)?;
        if external == "*" || external.trim().is_empty() {
            return Err(Error::Invalid(
                "the anonymous actor cannot be erased".into(),
            ));
        }
        let tx = metadata.db.begin().await?;
        lock_project(&tx, project).await?;
        let family = crate::identity::identity_family(&tx, project, external).await?;
        Self::bounded("erasure", self.client.query("ALTER TABLE events DELETE WHERE project_id=? AND external_id IN ? SETTINGS mutations_sync=2").bind(project).bind(family).execute()).await?;
        let tx = Arc::new(tx);
        let scoped = MetadataStore::from_transaction(tx.clone())?;
        let erased = PostgresAdapter
            .erase(&scoped, project, external, now)
            .await?;
        drop(scoped);
        Arc::try_unwrap(tx)
            .map_err(|_| Error::Invalid("erasure retained its metadata transaction".into()))?
            .commit()
            .await?;
        Ok(erased)
    }
    async fn refresh_reports(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<()> {
        Self::independent(metadata)?;
        self.compute_reports(metadata, project, now).await
    }
}
