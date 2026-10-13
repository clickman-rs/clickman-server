use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The adapter exposes semantic observations, not its physical chunk or table
/// representation. Only fields used by audience predicates need to be loaded.
pub struct ObservationQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub events: Vec<String>,
    pub fields: BTreeSet<String>,
}
pub(crate) fn select_fields(
    properties: Map<String, Value>,
    context: Map<String, Value>,
    fields: &BTreeSet<String>,
) -> BTreeMap<String, Value> {
    properties
        .into_iter()
        .chain(
            context
                .into_iter()
                .map(|(k, v)| (format!("context.{k}"), v)),
        )
        .filter(|(k, v)| fields.contains(k) && !v.is_null())
        .collect()
}
