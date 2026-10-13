mod reports;
mod rotation;

use super::AnalyticsAdapter;
use crate::{MetadataStore, Result, WriteIdentity, connection::Connection};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clickman_protocol::Event;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresAdapter;

struct PostgresContext<'a> {
    db: &'a Connection,
}

impl PostgresAdapter {
    fn context(metadata: &MetadataStore) -> PostgresContext<'_> {
        PostgresContext { db: &metadata.db }
    }
}

#[async_trait]
impl AnalyticsAdapter for PostgresAdapter {
    fn name(&self) -> &'static str {
        "postgres"
    }
    async fn observations(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        query: &crate::ObservationQuery,
    ) -> Result<Vec<clickman_analytics::Occurrence>> {
        let db = metadata.connection();
        let canonical = crate::identity::canonical_ids(db, project).await?;
        use crate::sql;
        use sea_orm::ConnectionTrait;
        let keys:std::collections::BTreeMap<i64,String>=db.query_all_raw(sql("SELECT id,key FROM clickman_keys WHERE project_id=$1 AND key IN (SELECT value FROM jsonb_array_elements_text($2))",vec![project.into(),serde_json::json!(query.fields).into()])).await?.into_iter().map(|r|Ok((r.try_get("","id")?,r.try_get("","key")?))).collect::<std::result::Result<_,sea_orm::DbErr>>()?;
        let mut cursor = (query.from.date_naive(), 0i64, 0i32);
        let mut result = Vec::new();
        loop {
            let chunks=db.query_all_raw(sql("SELECT c.day,c.event_name_id,c.seq,c.payload,n.name FROM clickman_chunks c JOIN clickman_event_names n ON n.project_id=c.project_id AND n.id=c.event_name_id WHERE c.project_id=$1 AND c.day BETWEEN $2 AND $3 AND n.name IN (SELECT value FROM jsonb_array_elements_text($4)) AND (c.day,c.event_name_id,c.seq)>($5,$6,$7) ORDER BY c.day,c.event_name_id,c.seq LIMIT 32",vec![project.into(),query.from.date_naive().into(),query.to.date_naive().into(),serde_json::json!(query.events).into(),cursor.0.into(),cursor.1.into(),cursor.2.into()])).await?;
            let size = chunks.len();
            for chunk in chunks {
                let day: chrono::NaiveDate = chunk.try_get("", "day")?;
                cursor = (
                    day,
                    chunk.try_get("", "event_name_id")?,
                    chunk.try_get("", "seq")?,
                );
                let name: String = chunk.try_get("", "name")?;
                for row in clickman_analytics::decode(&chunk.try_get::<Vec<u8>>("", "payload")?)? {
                    let at = day
                        .and_hms_opt(0, 0, 0)
                        .expect("UTC day")
                        .and_utc()
                        .timestamp_millis()
                        + row.millisecond;
                    if at < query.from.timestamp_millis() || at > query.to.timestamp_millis() {
                        continue;
                    }
                    let Some(actor) = canonical.get(&row.actor) else {
                        continue;
                    };
                    result.push(clickman_analytics::Occurrence {
                        actor: *actor,
                        at,
                        event: name.clone(),
                        fields: row
                            .fields
                            .into_iter()
                            .filter_map(|(k, v)| keys.get(&k).map(|name| (name.clone(), v)))
                            .collect(),
                    });
                }
            }
            if size < 32 {
                break;
            }
        }
        Ok(result)
    }
    async fn insert(
        &self,
        metadata: &MetadataStore,
        identity: &WriteIdentity,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        Self::context(metadata)
            .insert(identity, events, received_at)
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
        Self::context(metadata)
            .insert_server(project, source, events, received_at)
            .await
    }
    async fn rotate(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        Self::context(metadata).rotate(project, now).await
    }
    async fn prune(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        Self::context(metadata).prune(project, now).await
    }
    async fn erase(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        external: &str,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        Self::context(metadata).erase(project, external, now).await
    }
    async fn refresh_reports(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<()> {
        Self::context(metadata).refresh_reports(project, now).await
    }
}
