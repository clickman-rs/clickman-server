use crate::entities::destination;
use crate::{Error, MetadataStore, ProjectSettings, Result, lock_project, sql};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub type Destination = destination::Model;
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
        Ok(destination::Entity::find()
            .filter(destination::Column::ProjectId.eq(project))
            .order_by_asc(destination::Column::Name)
            .all(&self.db)
            .await?)
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
        let saved = destination::Entity::insert(destination::ActiveModel {
            id: Set(Uuid::new_v4()),
            project_id: Set(project),
            name: Set(name.into()),
            kind: Set(kind.into()),
            config: Set(config),
        })
        .on_conflict(
            OnConflict::columns([destination::Column::ProjectId, destination::Column::Name])
                .update_columns([destination::Column::Kind, destination::Column::Config])
                .to_owned(),
        )
        .exec_with_returning(&tx)
        .await?;
        // Set-based backfill stays in the same locked project transaction.
        tx.execute_raw(sql(
            "INSERT INTO clickman_deliveries(project_id,destination_id,message_id)
             SELECT $1,$2,message_id FROM clickman_events WHERE project_id=$1
             ON CONFLICT DO NOTHING",
            vec![project.into(), saved.id.into()],
        ))
        .await?;
        tx.commit().await?;
        Ok(saved)
    }
    pub async fn delete_destination(&self, project: Uuid, id: Uuid) -> Result<bool> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let deleted = destination::Entity::delete_many()
            .filter(destination::Column::ProjectId.eq(project))
            .filter(destination::Column::Id.eq(id))
            .exec(&tx)
            .await?
            .rows_affected
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
        let Some(destination) = destination::Entity::find_by_id(destination)
            .filter(destination::Column::ProjectId.eq(project))
            .one(&tx)
            .await?
        else {
            tx.commit().await?;
            return Ok(None);
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
