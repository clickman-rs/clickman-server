use super::{ClickHouseAdapter, NativeEvent};
use crate::{Error, MetadataStore, Result, lock_project, sql};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use clickhouse::Row;
use clickman_analytics::{Activity, Occurrence, actives, retention};
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Row, Deserialize)]
struct ActorDay {
    external_id: String,
    #[serde(with = "clickhouse::serde::chrono::date")]
    day: NaiveDate,
}

#[derive(Row, Deserialize)]
struct EventTotal {
    event: String,
    external_id: String,
    events: u64,
}

impl ClickHouseAdapter {
    pub(super) async fn compute_reports(
        &self,
        metadata: &MetadataStore,
        project: Uuid,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let tx = metadata.db.begin().await?;
        lock_project(&tx, project).await?;
        let today = now.date_naive();
        let first = today - Duration::days(119);
        let actors = tx.query_all_raw(sql("SELECT c.id,a.external_id,c.first_seen_on FROM clickman_actors a JOIN clickman_actors c ON c.project_id=a.project_id AND c.id=COALESCE(a.merged_into_id,a.id) WHERE a.project_id=$1 AND a.erased_at IS NULL AND c.erased_at IS NULL AND c.has_events AND c.external_id<>'*'",vec![project.into()])).await?;
        let mut activities: BTreeMap<String, Activity> = actors
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get("", "external_id")?,
                    Activity {
                        actor: row.try_get("", "id")?,
                        first_seen: row.try_get("", "first_seen_on")?,
                        days: BTreeSet::new(),
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let days = Self::bounded("activity", self.client.query("SELECT external_id,toDate(occurred_at) AS day FROM events FINAL WHERE project_id=? AND external_id<>'*' AND toDate(occurred_at) BETWEEN toDate(?) AND toDate(?) GROUP BY external_id,day").bind(project).bind(first.to_string()).bind(today.to_string()).fetch_all::<ActorDay>()).await?;
        for day in days {
            let activity = activities.get_mut(&day.external_id).ok_or_else(|| {
                Error::Invalid("analytics export requires a completed metadata rotation".into())
            })?;
            activity.days.insert(day.day);
        }
        let mut canonical: BTreeMap<i64, Activity> = BTreeMap::new();
        for item in activities.values() {
            let target = canonical.entry(item.actor).or_insert_with(|| Activity {
                actor: item.actor,
                first_seen: item.first_seen,
                days: BTreeSet::new(),
            });
            target.days.extend(&item.days);
        }
        let activity: Vec<_> = canonical.into_values().collect();
        save(&tx, project, "actives", actives(&activity, today), now).await?;
        save(&tx, project, "retention", retention(&activity, today), now).await?;
        let events = Self::bounded("event totals", self.client.query("SELECT event,external_id,count() AS events FROM events FINAL WHERE project_id=? AND toDate(occurred_at) BETWEEN toDate(?) AND toDate(?) GROUP BY event,external_id").bind(project).bind((today-Duration::days(29)).to_string()).bind(today.to_string()).fetch_all::<EventTotal>()).await?;
        let mut tallies: BTreeMap<String, (u64, BTreeSet<i64>)> = BTreeMap::new();
        for row in events {
            let tally = tallies.entry(row.event).or_default();
            tally.0 += row.events;
            if let Some(actor) = activities.get(&row.external_id) {
                tally.1.insert(actor.actor);
            }
        }
        let mut totals:Vec<_>=tallies.into_iter().map(|(event,(events,actors))|json!({"event":event,"events":events,"actors":actors.len()})).collect();
        totals.sort_by(|a, b| {
            b["events"]
                .as_u64()
                .cmp(&a["events"].as_u64())
                .then(a["event"].as_str().cmp(&b["event"].as_str()))
        });
        save(&tx, project, "events", json!({"events":totals}), now).await?;
        for funnel in metadata.funnels(project).await? {
            let rows = Self::bounded("funnel events", self.client.query("SELECT ?fields FROM events FINAL WHERE project_id=? AND external_id<>'*' AND event IN ? AND toDate(occurred_at) BETWEEN toDate(?) AND toDate(?) ORDER BY occurred_at,message_id,received_at").bind(project).bind(funnel.event_names()).bind(funnel.from(now).date_naive().to_string()).bind(today.to_string()).fetch_all::<NativeEvent>()).await?;
            let mut occurrences = Vec::with_capacity(rows.len());
            for row in rows {
                let actor = activities.get(&row.external_id).ok_or_else(|| {
                    Error::Invalid("analytics export requires a completed metadata rotation".into())
                })?;
                let properties: Map<String, Value> = serde_json::from_str(&row.properties)
                    .map_err(|e| Error::Invalid(e.to_string()))?;
                let context: Map<String, Value> = serde_json::from_str(&row.context)
                    .map_err(|e| Error::Invalid(e.to_string()))?;
                let fields = properties
                    .into_iter()
                    .chain(
                        context
                            .into_iter()
                            .map(|(key, value)| (format!("context.{key}"), value)),
                    )
                    .filter(|(_, value)| !value.is_null())
                    .collect();
                occurrences.push(Occurrence {
                    at: row.occurred_at.timestamp_millis(),
                    actor: actor.actor,
                    event: row.event,
                    fields,
                });
            }
            save(
                &tx,
                project,
                &format!("funnel:{}", funnel.key),
                funnel.compute(occurrences, now),
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

async fn save(
    db: &impl ConnectionTrait,
    project: Uuid,
    key: &str,
    result: Value,
    now: DateTime<Utc>,
) -> Result<()> {
    db.execute_raw(sql("INSERT INTO clickman_reports(project_id,key,result,computed_at) VALUES($1,$2,$3,$4) ON CONFLICT(project_id,key) DO UPDATE SET result=EXCLUDED.result,computed_at=EXCLUDED.computed_at",vec![project.into(),key.into(),result.into(),now.into()])).await?;
    Ok(())
}
