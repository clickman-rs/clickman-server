mod clickhouse;
mod postgres;

use crate::{MetadataStore, Result, WriteIdentity};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clickman_protocol::Event;
use uuid::Uuid;

pub use clickhouse::{ClickHouseAdapter, ClickHouseConfig};

/// Explicit operator configuration for the standalone binaries. Embedded hosts
/// can supply their own adapter directly instead of reading process environment.
pub fn from_environment() -> Result<std::sync::Arc<dyn AnalyticsAdapter>> {
    let value = setting("CLICKMAN_ANALYTICS_BACKEND", Some("postgres"))?;
    match value.as_str() {
        "postgres" => Ok(std::sync::Arc::new(PostgresAdapter)),
        "clickhouse" => {
            let url = setting("CLICKMAN_CLICKHOUSE_URL", None)?;
            let backend = ClickHouseAdapter::new(ClickHouseConfig {
                url,
                database: setting("CLICKMAN_CLICKHOUSE_DATABASE", Some("clickman"))?,
                user: setting("CLICKMAN_CLICKHOUSE_USER", Some("default"))?,
                password: setting("CLICKMAN_CLICKHOUSE_PASSWORD", Some(""))?,
            })?;
            Ok(std::sync::Arc::new(backend))
        }
        _ => Err(crate::Error::Invalid(
            "CLICKMAN_ANALYTICS_BACKEND must be postgres or clickhouse".into(),
        )),
    }
}
pub use postgres::PostgresAdapter;

fn setting(key: &str, fallback: Option<&str>) -> Result<String> {
    match std::env::var(key) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => fallback
            .map(str::to_owned)
            .ok_or_else(|| crate::Error::Invalid(format!("{key} is required"))),
        Err(_) => Err(crate::Error::Invalid(format!("{key} must be valid UTF-8"))),
    }
}

/// Analytics semantics, independent of the physical event representation.
///
/// Accepted batches must survive process failure, enforce current source-key
/// revocation and deduplicate message IDs within a project. Report calculations
/// share Clickman's exact actor, time-window and funnel semantics. The metadata
/// argument is request-scoped: PostgreSQL implementations must use its caller
/// transaction; remote backends must document their durable handoff guarantees.
#[async_trait]
pub trait AnalyticsAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    async fn observations(
        &self,
        _metadata: &MetadataStore,
        _project: Uuid,
        _query: &crate::ObservationQuery,
    ) -> Result<Vec<clickman_analytics::Occurrence>> {
        Err(crate::Error::Invalid(
            "this adapter does not implement segment observations".into(),
        ))
    }
    async fn migrate(&self, _metadata: &MetadataStore) -> Result<()> {
        Ok(())
    }
    async fn healthy(&self, _metadata: &MetadataStore) -> Result<()> {
        Ok(())
    }
    async fn insert(
        &self,
        metadata: &MetadataStore,
        identity: &WriteIdentity,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64>;
    async fn insert_server(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        source: &str,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64>;
    async fn rotate(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64>;
    async fn prune(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64>;
    async fn erase(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        external: &str,
        now: DateTime<Utc>,
    ) -> Result<bool>;
    async fn refresh_reports(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<()>;
}

#[cfg(test)]
mod tests;
