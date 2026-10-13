use crate::Engine;
use chrono::Utc;
use clickman_protocol::{Limits, Sanitizer, normalize};
use clickman_storage::{Error, ProjectSettings, Result};
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub struct TrackingEvent {
    pub message_id: Uuid,
    pub event: String,
    pub external_id: String,
    pub properties: Value,
    pub context: Value,
    pub timestamp: OffsetDateTime,
}
impl TrackingEvent {
    pub fn new(
        event: impl Into<String>,
        external_id: impl Into<String>,
        properties: Value,
    ) -> Self {
        Self {
            message_id: Uuid::now_v7(),
            event: event.into(),
            external_id: external_id.into(),
            properties,
            context: json!({}),
            timestamp: OffsetDateTime::now_utc(),
        }
    }
}
impl Engine {
    /// Trusted server-side tracking; the host supplies the authorized project.
    /// Set message_id before calling when a host operation needs idempotency.
    pub async fn track(&self, project: Uuid, source: &str, event: TrackingEvent) -> Result<bool> {
        if source.trim().is_empty() || source.len() > 200 {
            return Err(Error::Invalid("source must contain 1..200 bytes".into()));
        }
        let p = self.storage.project(project).await?;
        let settings: ProjectSettings =
            serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
        let now = OffsetDateTime::now_utc();
        let body = json!({"sentAt":now.format(&Rfc3339).unwrap(),"batch":[{"type":"track","messageId":event.message_id,"timestamp":event.timestamp.format(&Rfc3339).map_err(|e|Error::Invalid(e.to_string()))?,"event":event.event,"externalId":event.external_id,"properties":event.properties,"context":event.context}]});
        let normalized = normalize(
            &serde_json::to_vec(&body).unwrap(),
            now,
            &Sanitizer::new(&settings.filter_fragments),
            &Limits {
                max_event_age: time::Duration::days(settings.max_event_age_days.into()),
                ..Limits::default()
            },
        )
        .map_err(|e| Error::Invalid(e.to_string()))?;
        if !normalized.rejected.is_empty() {
            return Err(Error::Invalid(format!(
                "invalid server event: {:?}",
                normalized.rejected[0].reason
            )));
        }
        Ok(self
            .storage
            .insert_server(project, source, &normalized.events, Utc::now())
            .await?
            == 1)
    }
}
