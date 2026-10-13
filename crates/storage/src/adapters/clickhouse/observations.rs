use super::NativeEvent;
use crate::{Error, Result};
use clickhouse::query::Query;
use clickman_analytics::Occurrence;
use serde_json::{Map, Value};
use std::{collections::BTreeSet, time::Duration};

/// Stream native rows into semantic observations without retaining a second
/// raw-event vector. One deadline covers the complete cursor, not each page.
pub(super) async fn collect(
    request: Query,
    actor_id: impl Fn(&str) -> Option<i64>,
    fields: Option<&BTreeSet<String>>,
    operation: &'static str,
) -> Result<Vec<Occurrence>> {
    let mut cursor = request
        .fetch::<NativeEvent>()
        .map_err(|source| Error::Backend {
            operation,
            source: Box::new(source),
        })?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    let mut observations = Vec::new();
    loop {
        let row = tokio::time::timeout_at(deadline, cursor.next())
            .await
            .map_err(|source| Error::Backend {
                operation,
                source: Box::new(source),
            })?
            .map_err(|source| Error::Backend {
                operation,
                source: Box::new(source),
            })?;
        let Some(row) = row else { break };
        let actor = actor_id(&row.external_id).ok_or_else(|| {
            Error::Invalid("analytics export requires a completed metadata rotation".into())
        })?;
        let properties: Map<String, Value> = serde_json::from_str(&row.properties)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let context: Map<String, Value> = serde_json::from_str(&row.context)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let fields = if let Some(fields) = fields {
            crate::observations::select_fields(properties, context, fields)
        } else {
            // Reports preserve full-field tie fallback for breakdown choices.
            properties
                .into_iter()
                .chain(
                    context
                        .into_iter()
                        .map(|(key, value)| (format!("context.{key}"), value)),
                )
                .filter(|(_, value)| !value.is_null())
                .collect()
        };
        observations.push(Occurrence {
            actor,
            at: row.occurred_at.timestamp_millis(),
            event: row.event,
            fields,
        });
    }
    Ok(observations)
}
