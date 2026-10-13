use crate::{Error, MetadataStore, ObservationQuery, Result, Storage, lock_project, sql};
use chrono::{DateTime, Utc};
use clickman_analytics::{Funnel, Segment, SegmentRule};
use sea_orm::{ConnectionTrait, QueryResult, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedSegment {
    pub id: Uuid,
    pub definition: Segment,
    pub enabled: bool,
    pub fire_existing: bool,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
    pub evaluated_at: Option<DateTime<Utc>>,
    pub members: i64,
}
fn row(r: QueryResult) -> Result<SavedSegment> {
    Ok(SavedSegment {
        id: r.try_get("", "id")?,
        definition: serde_json::from_value(r.try_get("", "definition")?)
            .map_err(|e| Error::Invalid(e.to_string()))?,
        enabled: r.try_get("", "enabled")?,
        fire_existing: r.try_get("", "fire_existing")?,
        revision: r.try_get("", "revision")?,
        updated_at: r.try_get("", "updated_at")?,
        evaluated_at: r.try_get("", "evaluated_at")?,
        members: r.try_get("", "members")?,
    })
}
impl MetadataStore {
    pub async fn segments(&self, project: Uuid) -> Result<Vec<SavedSegment>> {
        self.db.query_all_raw(sql("SELECT s.*, (SELECT count(*) FROM clickman_segment_members m WHERE m.project_id=s.project_id AND m.segment_id=s.id AND m.revision=s.revision AND m.present) AS members FROM clickman_segments s WHERE s.project_id=$1 ORDER BY s.key",vec![project.into()])).await?.into_iter().map(row).collect()
    }
    pub async fn save_segment(
        &self,
        project: Uuid,
        value: Value,
        enabled: bool,
        fire_existing: bool,
    ) -> Result<SavedSegment> {
        let tx = Arc::new(self.db.begin().await?);
        lock_project(tx.as_ref(), project).await?;
        let scoped = MetadataStore::from_transaction(tx.clone())?;
        let segment =
            Segment::parse(value, &scoped.funnels(project).await?).map_err(Error::Invalid)?;
        let definition = serde_json::to_value(&segment).expect("segment definition");
        let current=tx.query_one_raw(sql("SELECT id,definition,enabled FROM clickman_segments WHERE project_id=$1 AND key=$2",vec![project.into(),segment.key.clone().into()])).await?;
        let mut reset = true;
        let id = if let Some(current) = current {
            reset = current.try_get::<Value>("", "definition")?["rule"] != definition["rule"]
                || current.try_get::<bool>("", "enabled")? != enabled;
            let id: Uuid = current.try_get("", "id")?;
            tx.execute_raw(sql("UPDATE clickman_segments SET definition=$3,enabled=$4,fire_existing=$5,revision=revision+CASE WHEN $6 THEN 1 ELSE 0 END,updated_at=CASE WHEN $6 THEN now() ELSE updated_at END,evaluated_at=CASE WHEN $6 THEN NULL ELSE evaluated_at END WHERE project_id=$1 AND id=$2",vec![project.into(),id.into(),definition.into(),enabled.into(),fire_existing.into(),reset.into()])).await?;
            id
        } else {
            let id = Uuid::now_v7();
            tx.execute_raw(sql("INSERT INTO clickman_segments(id,project_id,key,definition,enabled,fire_existing) VALUES($1,$2,$3,$4,$5,$6)",vec![id.into(),project.into(),segment.key.into(),definition.into(),enabled.into(),fire_existing.into()])).await?;
            id
        };
        if reset {
            tx.execute_raw(sql("UPDATE clickman_webhook_deliveries d SET status='canceled' FROM clickman_webhooks w WHERE d.project_id=$1 AND d.webhook_id=w.id AND w.segment_id=$2 AND d.status='pending'",vec![project.into(),id.into()])).await?;
        }
        let result = scoped
            .segments(project)
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .expect("saved segment");
        drop(scoped);
        Arc::try_unwrap(tx)
            .map_err(|_| Error::Invalid("segment transaction remained in use".into()))?
            .commit()
            .await?;
        Ok(result)
    }
    pub async fn delete_segment(&self, project: Uuid, id: Uuid) -> Result<bool> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let count = tx
            .execute_raw(sql(
                "DELETE FROM clickman_segments WHERE project_id=$1 AND id=$2",
                vec![project.into(), id.into()],
            ))
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(count > 0)
    }
}

fn fields(segment: &Segment, funnels: &[Funnel]) -> BTreeSet<String> {
    match &segment.rule {
        SegmentRule::Event { filter, .. } => filter.filters.keys().cloned().collect(),
        SegmentRule::InactiveAfter { entry, absent, .. } => entry
            .filters
            .keys()
            .chain(absent.filters.keys())
            .cloned()
            .collect(),
        SegmentRule::Funnel { funnel_key, .. } => funnels
            .iter()
            .find(|f| f.key == *funnel_key)
            .expect("validated funnel")
            .steps
            .iter()
            .flat_map(|s| s.filters.keys().cloned())
            .collect(),
    }
}
impl Storage {
    pub async fn evaluate_segments(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        let tx = Arc::new(self.connection().begin().await?);
        lock_project(tx.as_ref(), project).await?;
        let scoped = self.in_transaction(tx.clone())?;
        let segments: Vec<_> = scoped
            .segments(project)
            .await?
            .into_iter()
            .filter(|s| s.enabled)
            .collect();
        if segments.is_empty() {
            return Ok(0);
        }
        let funnels = scoped.funnels(project).await?;
        let mut from = now;
        let mut events = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for s in &segments {
            s.definition.validate(&funnels).map_err(Error::Invalid)?;
            from = from.min(s.definition.from(&funnels, now));
            events.extend(s.definition.event_names(&funnels));
            keys.extend(fields(&s.definition, &funnels));
        }
        let observations = self
            .adapter
            .observations(
                &scoped.metadata,
                project,
                &ObservationQuery {
                    from,
                    to: now,
                    events: events.into_iter().collect(),
                    fields: keys,
                },
            )
            .await?;
        let mut entries = 0;
        for s in segments {
            let candidates = s.definition.members(&observations, &funnels, now);
            let previous=tx.query_all_raw(sql("SELECT actor_id,present,entries,revision FROM clickman_segment_members WHERE project_id=$1 AND segment_id=$2",vec![project.into(),s.id.into()])).await?;
            let mut states: BTreeMap<i64, (bool, i32, i32)> = BTreeMap::new();
            for row in previous {
                states.insert(
                    row.try_get("", "actor_id")?,
                    (
                        row.try_get("", "present")?,
                        row.try_get("", "entries")?,
                        row.try_get("", "revision")?,
                    ),
                );
            }
            for (actor, (present, _, revision)) in &states {
                if *present && (*revision != s.revision || !candidates.contains_key(actor)) {
                    tx.execute_raw(sql("UPDATE clickman_segment_members SET present=false,evaluated_at=$4 WHERE project_id=$1 AND segment_id=$2 AND actor_id=$3",vec![project.into(),s.id.into(),(*actor).into(),now.into()])).await?;
                }
            }
            for (actor, qualified) in candidates {
                let previous = states.get(&actor);
                if previous
                    .is_some_and(|(present, _, revision)| *present && *revision == s.revision)
                {
                    continue;
                }
                let ordinal = previous
                    .filter(|(_, _, revision)| *revision == s.revision)
                    .map(|(_, count, _)| count + 1)
                    .unwrap_or(1);
                tx.execute_raw(sql("INSERT INTO clickman_segment_members(project_id,segment_id,actor_id,present,entries,revision,entered_at,evaluated_at) VALUES($1,$2,$3,true,$4,$5,$6,$6) ON CONFLICT(project_id,segment_id,actor_id) DO UPDATE SET present=true,entries=$4,revision=$5,entered_at=$6,evaluated_at=$6",vec![project.into(),s.id.into(),actor.into(),ordinal.into(),s.revision.into(),now.into()])).await?;
                // Suppress historical matches when a segment is first created
                // or its rule changes, unless backfill was explicitly enabled.
                if ordinal == 1 && !s.fire_existing && qualified < s.updated_at.timestamp_millis() {
                    continue;
                }
                let external:String=tx.query_one_raw(sql("SELECT external_id FROM clickman_actors WHERE project_id=$1 AND id=$2 AND erased_at IS NULL",vec![project.into(),actor.into()])).await?.expect("active segment actor").try_get("","external_id")?;
                let id = Uuid::now_v7();
                let payload = json!({"id":id,"type":"segment.entered","projectId":project,"segment":{"id":s.id,"key":s.definition.key,"name":s.definition.name,"revision":s.revision},"externalId":external,"enteredAt":now,"qualifiedAt":DateTime::from_timestamp_millis(qualified),"ordinal":ordinal});
                tx.execute_raw(sql("INSERT INTO clickman_segment_entries(id,project_id,segment_id,actor_id,ordinal,revision,payload,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",vec![id.into(),project.into(),s.id.into(),actor.into(),ordinal.into(),s.revision.into(),payload.into(),now.into()])).await?;
                tx.execute_raw(sql("INSERT INTO clickman_webhook_deliveries(id,project_id,webhook_id,entry_id,next_attempt_at) SELECT gen_random_uuid(),$1,w.id,$2,$4 FROM clickman_webhooks w WHERE w.project_id=$1 AND w.segment_id=$3 AND w.enabled ON CONFLICT DO NOTHING",vec![project.into(),id.into(),s.id.into(),now.into()])).await?;
                entries += 1;
            }
            tx.execute_raw(sql(
                "UPDATE clickman_segments SET evaluated_at=$3 WHERE project_id=$1 AND id=$2",
                vec![project.into(), s.id.into(), now.into()],
            ))
            .await?;
        }
        drop(scoped);
        Arc::try_unwrap(tx)
            .map_err(|_| Error::Invalid("segment evaluation transaction remained in use".into()))?
            .commit()
            .await?;
        Ok(entries)
    }
}
