use crate::{Error, ProjectSettings, Result, Storage, WriteIdentity, lock_project, sql, utc};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use clickman_analytics::{CHUNK_EVENTS, Row, decode, encode, partition};
use clickman_protocol::Event;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const ROTATION_PAGE: u64 = 20_000;

impl Storage {
    pub async fn insert(
        &self,
        identity: &WriteIdentity,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        self.insert_events(
            identity.project_id,
            &identity.source,
            Some(identity.key_id),
            events,
            received_at,
        )
        .await
    }
    /// Trusted Rust hosts authenticate and authorize before invoking this API.
    pub async fn insert_server(
        &self,
        project: Uuid,
        source: &str,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        self.insert_events(project, source, None, events, received_at)
            .await
    }
    async fn insert_events(
        &self,
        project: Uuid,
        source: &str,
        key: Option<Uuid>,
        events: &[Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        let tx = self.db.begin().await?;
        tx.query_one_raw(sql(
            "SELECT id FROM clickman_projects WHERE id=$1 FOR SHARE",
            vec![project.into()],
        ))
        .await?
        .ok_or_else(|| Error::Invalid("project not found".into()))?;
        if let Some(key) = key
            && tx.query_one_raw(sql("SELECT id FROM clickman_write_keys WHERE project_id=$1 AND id=$2 AND revoked_at IS NULL",vec![project.into(),key.into()])).await?.is_none() {
            return Err(Error::Invalid("write key revoked".into()));
        }
        let input:Vec<_>=events.iter().map(|e|json!({"message_id":e.message_id,"occurred_at":utc(e.occurred_at),"external_id":e.external_id,"event":e.event,"properties":e.properties,"context":e.context})).collect();
        let inserted=tx.query_all_raw(sql("INSERT INTO clickman_events(project_id,message_id,source,occurred_at,received_at,external_id,event,properties,context) SELECT $1,message_id,$2,occurred_at,$3,external_id,event,properties,context FROM jsonb_to_recordset($4) AS e(message_id uuid,occurred_at timestamptz,external_id text,event text,properties jsonb,context jsonb) ON CONFLICT(project_id,message_id) DO NOTHING RETURNING message_id",vec![project.into(),source.into(),received_at.into(),json!(input).into()])).await?;
        let ids: Vec<Uuid> = inserted
            .iter()
            .map(|r| r.try_get("", "message_id"))
            .collect::<std::result::Result<_, _>>()?;
        if !ids.is_empty() {
            tx.execute_raw(sql("INSERT INTO clickman_deliveries(project_id,destination_id,message_id) SELECT $1,d.id,j.value::uuid FROM clickman_destinations d CROSS JOIN jsonb_array_elements_text($2) j WHERE d.project_id=$1 ON CONFLICT DO NOTHING",vec![project.into(),json!(ids).into()])).await?;
        }
        tx.commit().await?;
        Ok(ids.len() as u64)
    }

    pub async fn rotate(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        let mut rotated = 0;
        loop {
            let tx = self.db.begin().await?;
            let p = lock_project(&tx, project).await?;
            let settings: ProjectSettings =
                serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
            let cutoff = now - Duration::seconds(settings.rotation_lag_seconds.into());
            let raw=tx.query_all_raw(sql("SELECT message_id,occurred_at,external_id,event,properties,context FROM clickman_events WHERE project_id=$1 AND rotated_at IS NULL AND received_at <= $2 ORDER BY received_at,message_id LIMIT 20000",vec![project.into(),cutoff.into()])).await?;
            if raw.is_empty() {
                tx.commit().await?;
                break;
            }
            let mut names = BTreeSet::new();
            let mut keys = BTreeSet::new();
            let mut first_days: BTreeMap<String, NaiveDate> = BTreeMap::new();
            let mut parsed = Vec::with_capacity(raw.len());
            for r in raw {
                let message_id: Uuid = r.try_get("", "message_id")?;
                let at: DateTime<Utc> = r.try_get("", "occurred_at")?;
                let external: String = r.try_get("", "external_id")?;
                let event: String = r.try_get("", "event")?;
                let properties: Value = r.try_get("", "properties")?;
                let context: Value = r.try_get("", "context")?;
                let fields: BTreeMap<String, Value> = properties
                    .as_object()
                    .ok_or_else(|| Error::Invalid("invalid stored properties".into()))?
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .chain(
                        context
                            .as_object()
                            .ok_or_else(|| Error::Invalid("invalid stored context".into()))?
                            .iter()
                            .map(|(k, v)| (format!("context.{k}"), v.clone())),
                    )
                    .filter(|(_, v)| !v.is_null())
                    .collect();
                names.insert(event.clone());
                keys.extend(fields.keys().cloned());
                first_days
                    .entry(external.clone())
                    .and_modify(|d| *d = (*d).min(at.date_naive()))
                    .or_insert(at.date_naive());
                parsed.push((message_id, at, external, event, fields));
            }
            let names = dictionary(&tx, project, "clickman_event_names", "name", names).await?;
            let keys = dictionary(&tx, project, "clickman_keys", "key", keys).await?;
            let input:Vec<_>=first_days.into_iter().map(|(external_id,first_seen_on)|json!({"external_id":external_id,"first_seen_on":first_seen_on})).collect();
            let actor_rows=tx.query_all_raw(sql("INSERT INTO clickman_actors(project_id,external_id,first_seen_on) SELECT $1,external_id,first_seen_on FROM jsonb_to_recordset($2) AS a(external_id text,first_seen_on date) ON CONFLICT(project_id,external_id) DO UPDATE SET first_seen_on=LEAST(clickman_actors.first_seen_on,EXCLUDED.first_seen_on) RETURNING id,external_id",vec![project.into(),json!(input).into()])).await?;
            let actors: BTreeMap<String, i64> = actor_rows
                .into_iter()
                .map(|r| Ok((r.try_get("", "external_id")?, r.try_get("", "id")?)))
                .collect::<std::result::Result<_, sea_orm::DbErr>>()?;
            let mut groups: BTreeMap<(NaiveDate, i64), Vec<Row>> = BTreeMap::new();
            let mut bits: BTreeMap<(i64, i32), Vec<u8>> = BTreeMap::new();
            for (_, at, external, event, fields) in &parsed {
                let day = at.date_naive();
                let actor = actors[external];
                let millis = at.timestamp_millis()
                    - day
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp_millis();
                groups.entry((day, names[event])).or_default().push(Row {
                    millisecond: millis,
                    actor,
                    fields: fields.iter().map(|(k, v)| (keys[k], v.clone())).collect(),
                });
                if external != "*" {
                    bits.entry((actor, day.year()))
                        .or_insert_with(|| vec![b'0'; 366])[day.ordinal0() as usize] = b'1';
                }
            }
            for ((day, event), rows) in &groups {
                append(&tx, project, *day, *event, rows.clone()).await?;
                recount(&tx, project, *day, *event).await?;
            }
            for ((actor, year), days) in bits {
                let days = String::from_utf8(days).expect("ascii bits");
                tx.execute_raw(sql("INSERT INTO clickman_actor_days(project_id,actor_id,year,days) VALUES($1,$2,$3,$4::varbit) ON CONFLICT(project_id,actor_id,year) DO UPDATE SET days=clickman_actor_days.days | EXCLUDED.days",vec![project.into(),actor.into(),year.into(),days.into()])).await?;
            }
            let ids: Vec<_> = parsed.iter().map(|p| p.0).collect();
            tx.execute_raw(sql("UPDATE clickman_events SET rotated_at=$3 WHERE project_id=$1 AND message_id IN (SELECT value::uuid FROM jsonb_array_elements_text($2))",vec![project.into(),json!(ids).into(),now.into()])).await?;
            let count = parsed.len() as u64;
            tx.commit().await?;
            rotated += count;
            if count < ROTATION_PAGE {
                break;
            }
        }
        Ok(rotated)
    }

    pub async fn prune(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        let tx = self.db.begin().await?;
        let p = lock_project(&tx, project).await?;
        let settings: ProjectSettings =
            serde_json::from_value(p.settings).map_err(|e| Error::Invalid(e.to_string()))?;
        let deleted=tx.execute_raw(sql("DELETE FROM clickman_events e WHERE e.project_id=$1 AND e.rotated_at IS NOT NULL AND e.received_at < $2 AND NOT EXISTS(SELECT 1 FROM clickman_deliveries d WHERE d.project_id=e.project_id AND d.message_id=e.message_id AND d.completed_at IS NULL)",vec![project.into(),(now-Duration::days(settings.dedup_window_days.into())).into()])).await?.rows_affected();
        if let Some(retention) = settings.retention_days {
            let boundary = (now - Duration::days(retention.into()))
                .date_naive()
                .with_day(1)
                .unwrap();
            tx.execute_raw(sql(
                "DELETE FROM clickman_chunks WHERE project_id=$1 AND day<$2",
                vec![project.into(), boundary.into()],
            ))
            .await?;
            tx.execute_raw(sql(
                "DELETE FROM clickman_daily_counts WHERE project_id=$1 AND day<$2",
                vec![project.into(), boundary.into()],
            ))
            .await?;
            tx.execute_raw(sql(
                "DELETE FROM clickman_actor_days WHERE project_id=$1 AND year<$2",
                vec![project.into(), boundary.year().into()],
            ))
            .await?;
            let prefix = boundary.ordinal0() as usize;
            let mask = format!("{}{}", "0".repeat(prefix), "1".repeat(366 - prefix));
            tx.execute_raw(sql("UPDATE clickman_actor_days SET days=days & $3::varbit WHERE project_id=$1 AND year=$2",vec![project.into(),boundary.year().into(),mask.into()])).await?;
            tx.execute_raw(sql(
                "DELETE FROM clickman_actor_days WHERE project_id=$1 AND bit_count(days)=0",
                vec![project.into()],
            ))
            .await?;
        }
        tx.commit().await?;
        Ok(deleted)
    }

    pub async fn erase(&self, project: Uuid, external: &str, now: DateTime<Utc>) -> Result<bool> {
        if external == "*" || external.trim().is_empty() {
            return Err(Error::Invalid(
                "the anonymous actor cannot be erased".into(),
            ));
        }
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let raw = tx
            .execute_raw(sql(
                "DELETE FROM clickman_events WHERE project_id=$1 AND external_id=$2",
                vec![project.into(), external.into()],
            ))
            .await?
            .rows_affected();
        let actor = tx
            .query_one_raw(sql(
                "SELECT id FROM clickman_actors WHERE project_id=$1 AND external_id=$2",
                vec![project.into(), external.into()],
            ))
            .await?;
        if let Some(ref actor) = actor {
            let actor: i64 = actor.try_get("", "id")?;
            let activity=tx.query_all_raw(sql("SELECT year,days::text AS days FROM clickman_actor_days WHERE project_id=$1 AND actor_id=$2",vec![project.into(),actor.into()])).await?;
            for a in activity {
                let year: i32 = a.try_get("", "year")?;
                let days: String = a.try_get("", "days")?;
                let january = NaiveDate::from_ymd_opt(year, 1, 1)
                    .ok_or_else(|| Error::Invalid("invalid activity year".into()))?;
                for (offset, bit) in days.bytes().enumerate().filter(|(_, b)| *b == b'1') {
                    let _ = bit;
                    let day = january + Duration::days(offset as i64);
                    let chunks=tx.query_all_raw(sql("SELECT event_name_id,seq,payload FROM clickman_chunks WHERE project_id=$1 AND day=$2",vec![project.into(),day.into()])).await?;
                    let mut touched = BTreeSet::new();
                    for chunk in chunks {
                        let event: i64 = chunk.try_get("", "event_name_id")?;
                        let seq: i32 = chunk.try_get("", "seq")?;
                        let payload: Vec<u8> = chunk.try_get("", "payload")?;
                        let rows = decode(&payload)?;
                        let kept: Vec<_> =
                            rows.iter().filter(|r| r.actor != actor).cloned().collect();
                        if kept.len() == rows.len() {
                            continue;
                        }
                        if kept.is_empty() {
                            tx.execute_raw(sql("DELETE FROM clickman_chunks WHERE project_id=$1 AND day=$2 AND event_name_id=$3 AND seq=$4",vec![project.into(),day.into(),event.into(),seq.into()])).await?;
                        } else {
                            save_chunk(&tx, project, day, event, seq, &kept).await?;
                        }
                        touched.insert(event);
                    }
                    for event in touched {
                        recount(&tx, project, day, event).await?;
                    }
                }
            }
            tx.execute_raw(sql(
                "DELETE FROM clickman_actor_days WHERE project_id=$1 AND actor_id=$2",
                vec![project.into(), actor.into()],
            ))
            .await?;
            tx.execute_raw(sql("UPDATE clickman_actors SET external_id=NULL,erased_at=$3 WHERE project_id=$1 AND id=$2",vec![project.into(),actor.into(),now.into()])).await?;
        }
        tx.execute_raw(sql(
            "DELETE FROM clickman_reports WHERE project_id=$1",
            vec![project.into()],
        ))
        .await?;
        tx.commit().await?;
        Ok(raw > 0 || actor.is_some())
    }
}

async fn dictionary(
    db: &impl ConnectionTrait,
    project: Uuid,
    table: &str,
    column: &str,
    keys: BTreeSet<String>,
) -> Result<BTreeMap<String, i64>> {
    let values = json!(keys);
    db.execute_raw(sql(&format!("INSERT INTO {table}(project_id,{column}) SELECT $1,value FROM jsonb_array_elements_text($2) ON CONFLICT(project_id,{column}) DO NOTHING"),vec![project.into(),values.clone().into()])).await?;
    db.query_all_raw(sql(&format!("SELECT id,{column} AS value FROM {table} WHERE project_id=$1 AND {column} IN (SELECT value FROM jsonb_array_elements_text($2))"),vec![project.into(),values.into()])).await?.into_iter().map(|r|Ok((r.try_get("","value")?,r.try_get("","id")?))).collect()
}

async fn append(
    db: &impl ConnectionTrait,
    project: Uuid,
    day: NaiveDate,
    event: i64,
    mut rows: Vec<Row>,
) -> Result<()> {
    let last=db.query_one_raw(sql("SELECT seq,events,payload FROM clickman_chunks WHERE project_id=$1 AND day=$2 AND event_name_id=$3 ORDER BY seq DESC LIMIT 1",vec![project.into(),day.into(),event.into()])).await?;
    let mut seq = 1i32;
    if let Some(last) = last {
        seq = last.try_get("", "seq")?;
        if last.try_get::<i32>("", "events")? < CHUNK_EVENTS as i32 {
            let payload: Vec<u8> = last.try_get("", "payload")?;
            let mut existing = decode(&payload)?;
            existing.append(&mut rows);
            rows = existing;
        } else {
            seq += 1;
        }
    }
    rows.sort_by_key(|r| r.millisecond);
    for (offset, chunk) in partition(&rows)?.into_iter().enumerate() {
        save_chunk(db, project, day, event, seq + offset as i32, chunk).await?;
    }
    Ok(())
}

async fn save_chunk(
    db: &impl ConnectionTrait,
    project: Uuid,
    day: NaiveDate,
    event: i64,
    seq: i32,
    rows: &[Row],
) -> Result<()> {
    let keys: BTreeSet<i64> = rows.iter().flat_map(|r| r.fields.keys().copied()).collect();
    let payload = encode(rows)?;
    db.execute_raw(sql("INSERT INTO clickman_chunks(project_id,day,event_name_id,seq,events,key_ids,payload) VALUES($1,$2,$3,$4,$5,ARRAY(SELECT value::bigint FROM jsonb_array_elements_text($6)),$7) ON CONFLICT(project_id,day,event_name_id,seq) DO UPDATE SET events=EXCLUDED.events,key_ids=EXCLUDED.key_ids,payload=EXCLUDED.payload",vec![project.into(),day.into(),event.into(),seq.into(),(rows.len() as i32).into(),json!(keys).into(),payload.into()])).await?;
    Ok(())
}

async fn recount(
    db: &impl ConnectionTrait,
    project: Uuid,
    day: NaiveDate,
    event: i64,
) -> Result<()> {
    let chunks=db.query_all_raw(sql("SELECT payload FROM clickman_chunks WHERE project_id=$1 AND day=$2 AND event_name_id=$3",vec![project.into(),day.into(),event.into()])).await?;
    let anonymous = db
        .query_one_raw(sql(
            "SELECT id FROM clickman_actors WHERE project_id=$1 AND external_id='*'",
            vec![project.into()],
        ))
        .await?
        .map(|r| r.try_get::<i64>("", "id"))
        .transpose()?;
    let mut events = 0i64;
    let mut actors = BTreeSet::new();
    for c in chunks {
        let payload: Vec<u8> = c.try_get("", "payload")?;
        let rows = decode(&payload)?;
        events += rows.len() as i64;
        actors.extend(
            rows.iter()
                .map(|r| r.actor)
                .filter(|a| Some(*a) != anonymous),
        );
    }
    if events == 0 {
        db.execute_raw(sql(
            "DELETE FROM clickman_daily_counts WHERE project_id=$1 AND day=$2 AND event_name_id=$3",
            vec![project.into(), day.into(), event.into()],
        ))
        .await?;
    } else {
        db.execute_raw(sql("INSERT INTO clickman_daily_counts(project_id,day,event_name_id,events,actors) VALUES($1,$2,$3,$4,$5) ON CONFLICT(project_id,day,event_name_id) DO UPDATE SET events=EXCLUDED.events,actors=EXCLUDED.actors",vec![project.into(),day.into(),event.into(),events.into(),(actors.len() as i64).into()])).await?;
    }
    Ok(())
}
