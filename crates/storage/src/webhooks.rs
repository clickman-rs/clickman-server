use crate::{Error, MetadataStore, Result, lock_project, sql};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Webhook {
    pub id: Uuid,
    pub project_id: Uuid,
    pub segment_id: Uuid,
    pub name: String,
    pub kind: String,
    pub config: Value,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebhookLog {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub entry_id: Uuid,
    pub status: String,
    pub attempts: i32,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub next_attempt_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
fn webhook(row: sea_orm::QueryResult) -> Result<Webhook> {
    Ok(Webhook {
        id: row.try_get("", "id")?,
        project_id: row.try_get("", "project_id")?,
        segment_id: row.try_get("", "segment_id")?,
        name: row.try_get("", "name")?,
        kind: row.try_get("", "kind")?,
        config: row.try_get("", "config")?,
        enabled: row.try_get("", "enabled")?,
    })
}
/// As for event destinations, the project lock protects a delivery already
/// handed to the network against erasure and removal until its result commits.
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook: Webhook,
    pub payload: Value,
    pub attempts: i32,
    tx: DatabaseTransaction,
}
impl WebhookDelivery {
    pub async fn complete(self, status: u16, now: DateTime<Utc>) -> Result<()> {
        self.tx.execute_raw(sql("UPDATE clickman_webhook_deliveries SET status='delivered',attempts=attempts+1,last_status=$3,last_error=NULL,completed_at=$4 WHERE project_id=$1 AND id=$2",vec![self.webhook.project_id.into(),self.id.into(),i32::from(status).into(),now.into()])).await?;
        self.tx.commit().await?;
        Ok(())
    }
    pub async fn retry(self, status: Option<u16>, message: &str, now: DateTime<Utc>) -> Result<()> {
        self.tx.execute_raw(sql("UPDATE clickman_webhook_deliveries SET status=CASE WHEN attempts>=11 THEN 'failed' ELSE 'pending' END,attempts=attempts+1,last_status=$3,last_error=$4,next_attempt_at=$5+make_interval(secs=>LEAST(3600,5*power(2,LEAST(attempts,10)))::int) WHERE project_id=$1 AND id=$2",vec![self.webhook.project_id.into(),self.id.into(),status.map(i32::from).into(),message.chars().take(200).collect::<String>().into(),now.into()])).await?;
        self.tx.commit().await?;
        Ok(())
    }
}
impl MetadataStore {
    pub async fn webhooks(&self, project: Uuid) -> Result<Vec<Webhook>> {
        self.db
            .query_all_raw(sql(
                "SELECT * FROM clickman_webhooks WHERE project_id=$1 ORDER BY name,id",
                vec![project.into()],
            ))
            .await?
            .into_iter()
            .map(webhook)
            .collect()
    }
    pub async fn save_webhook(
        &self,
        project: Uuid,
        segment: Uuid,
        name: &str,
        kind: &str,
        config: Value,
        enabled: bool,
    ) -> Result<Webhook> {
        if name.trim().is_empty() || name.len() > 100 || !matches!(kind, "webhook" | "zapier") {
            return Err(Error::Invalid("invalid webhook name or kind".into()));
        }
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        tx.query_one_raw(sql(
            "SELECT id FROM clickman_segments WHERE project_id=$1 AND id=$2",
            vec![project.into(), segment.into()],
        ))
        .await?
        .ok_or_else(|| Error::Invalid("segment not found in project".into()))?;
        let saved=tx.query_one_raw(sql("INSERT INTO clickman_webhooks(id,project_id,segment_id,name,kind,config,enabled) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(project_id,segment_id,name) DO UPDATE SET kind=$5,config=$6,enabled=$7 RETURNING *",vec![Uuid::now_v7().into(),project.into(),segment.into(),name.into(),kind.into(),config.into(),enabled.into()])).await?.expect("saved webhook");
        let result = webhook(saved)?;
        if !enabled {
            tx.execute_raw(sql("UPDATE clickman_webhook_deliveries SET status='canceled' WHERE project_id=$1 AND webhook_id=$2 AND status='pending'",vec![project.into(),result.id.into()])).await?;
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn delete_webhook(&self, project: Uuid, id: Uuid) -> Result<bool> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let count = tx
            .execute_raw(sql(
                "DELETE FROM clickman_webhooks WHERE project_id=$1 AND id=$2",
                vec![project.into(), id.into()],
            ))
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(count > 0)
    }
    pub async fn webhook_logs(&self, project: Uuid, limit: u64) -> Result<Vec<WebhookLog>> {
        if limit == 0 || limit > 100 {
            return Err(Error::Invalid("log limit must be 1–100".into()));
        }
        self.db.query_all_raw(sql("SELECT * FROM clickman_webhook_deliveries WHERE project_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2",vec![project.into(),(limit as i64).into()])).await?.into_iter().map(|r|Ok(WebhookLog{id:r.try_get("","id")?,webhook_id:r.try_get("","webhook_id")?,entry_id:r.try_get("","entry_id")?,status:r.try_get("","status")?,attempts:r.try_get("","attempts")?,last_status:r.try_get("","last_status")?,last_error:r.try_get("","last_error")?,next_attempt_at:r.try_get("","next_attempt_at")?,completed_at:r.try_get("","completed_at")?,created_at:r.try_get("","created_at")?})).collect()
    }
    pub async fn replay_webhook(&self, project: Uuid, id: Uuid) -> Result<bool> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let count=tx.execute_raw(sql("UPDATE clickman_webhook_deliveries d SET status='pending',attempts=0,next_attempt_at=now(),completed_at=NULL,last_error=NULL FROM clickman_webhooks w JOIN clickman_segments s ON s.project_id=w.project_id AND s.id=w.segment_id WHERE d.project_id=$1 AND d.id=$2 AND d.webhook_id=w.id AND w.enabled AND s.enabled",vec![project.into(),id.into()])).await?.rows_affected();
        tx.commit().await?;
        Ok(count > 0)
    }
    pub async fn webhook_delivery(
        &self,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<WebhookDelivery>> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let Some(row)=tx.query_one_raw(sql("SELECT d.id AS delivery_id,d.attempts,e.payload,w.* FROM clickman_webhook_deliveries d JOIN clickman_webhooks w ON w.project_id=d.project_id AND w.id=d.webhook_id JOIN clickman_segments s ON s.project_id=w.project_id AND s.id=w.segment_id JOIN clickman_segment_entries e ON e.project_id=d.project_id AND e.id=d.entry_id WHERE d.project_id=$1 AND d.status='pending' AND d.next_attempt_at<=$2 AND w.enabled AND s.enabled ORDER BY d.created_at,d.id LIMIT 1",vec![project.into(),now.into()])).await? else {tx.commit().await?;return Ok(None);};
        let id = row.try_get("", "delivery_id")?;
        let attempts = row.try_get("", "attempts")?;
        let payload = row.try_get("", "payload")?;
        let webhook = webhook(row)?;
        Ok(Some(WebhookDelivery {
            id,
            webhook,
            payload,
            attempts,
            tx,
        }))
    }
}
