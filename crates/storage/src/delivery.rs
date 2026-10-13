use crate::{Error, MetadataStore, ProjectSettings, Result, lock_project, sql};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Destination {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub kind: String,
    pub config: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub message_id: Uuid,
    pub source: String,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub external_id: String,
    pub event: String,
    pub properties: Value,
    pub context: Value,
}

/// Holds the project lock until success/retry is committed. Erasure and removal
/// cannot race a page that has already been handed to an external destination.
pub struct DeliveryPage {
    pub destination: Destination,
    pub events: Vec<StoredEvent>,
    tx: DatabaseTransaction,
}
impl DeliveryPage {
    pub async fn complete(self, now: DateTime<Utc>) -> Result<()> {
        let ids: Vec<_> = self.events.iter().map(|e| e.message_id).collect();
        self.tx.execute_raw(sql("UPDATE clickman_deliveries SET completed_at=$4,last_error=NULL WHERE project_id=$1 AND destination_id=$2 AND message_id IN (SELECT value::uuid FROM jsonb_array_elements_text($3))",vec![self.destination.project_id.into(),self.destination.id.into(),json!(ids).into(),now.into()])).await?;
        self.tx.commit().await?;
        Ok(())
    }
    pub async fn retry(self, now: DateTime<Utc>, error: &str) -> Result<()> {
        let ids: Vec<_> = self.events.iter().map(|e| e.message_id).collect();
        self.tx.execute_raw(sql("UPDATE clickman_deliveries SET attempts=attempts+1,last_error=$4,next_attempt_at=$5 + (LEAST(3600,30*power(2,LEAST(attempts,7)))::int * interval '1 second') WHERE project_id=$1 AND destination_id=$2 AND message_id IN (SELECT value::uuid FROM jsonb_array_elements_text($3))",vec![self.destination.project_id.into(),self.destination.id.into(),json!(ids).into(),error.chars().take(500).collect::<String>().into(),now.into()])).await?;
        self.tx.commit().await?;
        Ok(())
    }
}

impl MetadataStore {
    pub async fn destinations(&self, project: Uuid) -> Result<Vec<Destination>> {
        self.db.query_all_raw(sql("SELECT id,project_id,name,kind,config FROM clickman_destinations WHERE project_id=$1 ORDER BY name",vec![project.into()])).await?.into_iter().map(|r|Ok(Destination{id:r.try_get("","id")?,project_id:r.try_get("","project_id")?,name:r.try_get("","name")?,kind:r.try_get("","kind")?,config:r.try_get("","config")?})).collect()
    }
    pub async fn save_destination(
        &self,
        project: Uuid,
        name: &str,
        kind: &str,
        config: Value,
    ) -> Result<Destination> {
        if kind.trim().is_empty() || kind.len() > 100 || name.trim().is_empty() || name.len() > 100
        {
            return Err(Error::Invalid("invalid destination name or kind".into()));
        }
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let row=tx.query_one_raw(sql("INSERT INTO clickman_destinations(id,project_id,name,kind,config) VALUES($1,$2,$3,$4,$5) ON CONFLICT(project_id,name) DO UPDATE SET kind=EXCLUDED.kind,config=EXCLUDED.config RETURNING id",vec![Uuid::new_v4().into(),project.into(),name.into(),kind.into(),config.clone().into()])).await?.expect("RETURNING destination");
        let id: Uuid = row.try_get("", "id")?;
        tx.execute_raw(sql("INSERT INTO clickman_deliveries(project_id,destination_id,message_id) SELECT $1,$2,message_id FROM clickman_events WHERE project_id=$1 ON CONFLICT DO NOTHING",vec![project.into(),id.into()])).await?;
        tx.commit().await?;
        Ok(Destination {
            id,
            project_id: project,
            name: name.into(),
            kind: kind.into(),
            config,
        })
    }
    pub async fn delete_destination(&self, project: Uuid, id: Uuid) -> Result<bool> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let deleted = tx
            .execute_raw(sql(
                "DELETE FROM clickman_destinations WHERE project_id=$1 AND id=$2",
                vec![project.into(), id.into()],
            ))
            .await?
            .rows_affected()
            > 0;
        tx.commit().await?;
        Ok(deleted)
    }
    pub async fn delivery_page(
        &self,
        project: Uuid,
        destination: Uuid,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Option<DeliveryPage>> {
        if limit == 0 || limit > 2000 {
            return Err(Error::Invalid("delivery page limit must be 1..2000".into()));
        }
        let tx = self.db.begin().await?;
        let p = lock_project(&tx, project).await?;
        let settings: ProjectSettings =
            serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
        let Some(d)=tx.query_one_raw(sql("SELECT id,project_id,name,kind,config FROM clickman_destinations WHERE project_id=$1 AND id=$2",vec![project.into(),destination.into()])).await? else {tx.commit().await?; return Ok(None)};
        let destination = Destination {
            id: d.try_get("", "id")?,
            project_id: project,
            name: d.try_get("", "name")?,
            kind: d.try_get("", "kind")?,
            config: d.try_get("", "config")?,
        };
        let rows=tx.query_all_raw(sql("SELECT e.message_id,e.source,e.occurred_at,e.received_at,e.external_id,e.event,e.properties,e.context FROM clickman_events e JOIN clickman_deliveries d ON d.project_id=e.project_id AND d.message_id=e.message_id WHERE e.project_id=$1 AND d.destination_id=$2 AND d.completed_at IS NULL AND d.next_attempt_at<=$3 AND e.received_at<=$4 ORDER BY e.received_at,e.message_id LIMIT $5",vec![project.into(),destination.id.into(),now.into(),(now-Duration::seconds(settings.rotation_lag_seconds.into())).into(),i64::from(limit).into()])).await?;
        let events: Vec<StoredEvent> = rows
            .into_iter()
            .map(|r| {
                Ok(StoredEvent {
                    message_id: r.try_get("", "message_id")?,
                    source: r.try_get("", "source")?,
                    occurred_at: r.try_get("", "occurred_at")?,
                    received_at: r.try_get("", "received_at")?,
                    external_id: r.try_get("", "external_id")?,
                    event: r.try_get("", "event")?,
                    properties: r.try_get("", "properties")?,
                    context: r.try_get("", "context")?,
                })
            })
            .collect::<Result<_>>()?;
        if events.is_empty() {
            tx.commit().await?;
            return Ok(None);
        }
        Ok(Some(DeliveryPage {
            destination,
            events,
            tx,
        }))
    }
}
