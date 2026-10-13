use crate::{Error, Result, Storage, lock_project, sql};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use clickman_analytics::{Activity, Funnel, Occurrence, actives, decode, retention};
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub key: String,
    pub result: Value,
    pub computed_at: DateTime<Utc>,
}

impl Storage {
    pub async fn reports(&self, project: Uuid) -> Result<Vec<Report>> {
        self.db.query_all_raw(sql("SELECT key,result,computed_at FROM clickman_reports WHERE project_id=$1 ORDER BY key",vec![project.into()])).await?.into_iter().map(|r|Ok(Report{key:r.try_get("","key")?,result:r.try_get("","result")?,computed_at:r.try_get("","computed_at")?})).collect()
    }
    pub async fn refresh_reports(&self, project: Uuid, now: DateTime<Utc>) -> Result<()> {
        let tx = self.db.begin().await?;
        lock_project(&tx, project).await?;
        let today = now.date_naive();
        let first = today - Duration::days(119);
        let rows=tx.query_all_raw(sql("SELECT a.id,a.first_seen_on,d.year,d.days::text AS days FROM clickman_actors a LEFT JOIN clickman_actor_days d ON d.project_id=a.project_id AND d.actor_id=a.id AND d.year BETWEEN $3 AND $4 WHERE a.project_id=$1 AND a.erased_at IS NULL AND a.external_id <> '*' AND (a.first_seen_on >= $2 OR d.actor_id IS NOT NULL)",vec![project.into(),(today-Duration::days(89)).into(),first.year().into(),today.year().into()])).await?;
        let mut activity: BTreeMap<i64, Activity> = BTreeMap::new();
        for row in rows {
            let actor: i64 = row.try_get("", "id")?;
            let first_seen: NaiveDate = row.try_get("", "first_seen_on")?;
            let item = activity.entry(actor).or_insert_with(|| Activity {
                actor,
                first_seen,
                days: BTreeSet::new(),
            });
            if let Some(year) = row.try_get::<Option<i32>>("", "year")? {
                let days: String = row.try_get("", "days")?;
                let january = NaiveDate::from_ymd_opt(year, 1, 1)
                    .ok_or_else(|| Error::Invalid("invalid activity year".into()))?;
                item.days.extend(
                    days.bytes()
                        .enumerate()
                        .filter(|(_, b)| *b == b'1')
                        .map(|(i, _)| january + Duration::days(i as i64))
                        .filter(|d| (first..=today).contains(d)),
                );
            }
        }
        let activity: Vec<_> = activity.into_values().collect();
        save(&tx, project, "actives", actives(&activity, today), now).await?;
        save(&tx, project, "retention", retention(&activity, today), now).await?;
        let funnels: Vec<Funnel> = tx
            .query_all_raw(sql(
                "SELECT definition FROM clickman_funnels WHERE project_id=$1 ORDER BY key",
                vec![project.into()],
            ))
            .await?
            .into_iter()
            .map(|r| {
                Funnel::parse(r.try_get("", "definition")?)
                    .map_err(|e| Error::Invalid(e.to_string()))
            })
            .collect::<Result<_>>()?;
        let from = funnels
            .iter()
            .map(|f| f.from(now).date_naive())
            .min()
            .unwrap_or(today)
            .min(today - Duration::days(29));
        let keys: BTreeMap<i64, String> = tx
            .query_all_raw(sql(
                "SELECT id,key FROM clickman_keys WHERE project_id=$1",
                vec![project.into()],
            ))
            .await?
            .into_iter()
            .map(|r| Ok((r.try_get("", "id")?, r.try_get("", "key")?)))
            .collect::<std::result::Result<_, sea_orm::DbErr>>()?;
        let anonymous = tx
            .query_one_raw(sql(
                "SELECT id FROM clickman_actors WHERE project_id=$1 AND external_id='*'",
                vec![project.into()],
            ))
            .await?
            .map(|r| r.try_get::<i64>("", "id"))
            .transpose()?;
        let mut totals: BTreeMap<String, (u64, BTreeSet<i64>)> = BTreeMap::new();
        let mut journeys: Vec<Vec<Occurrence>> = vec![vec![]; funnels.len()];
        let mut cursor = (from, 0i64, 0i32);
        loop {
            let page=tx.query_all_raw(sql("SELECT c.day,c.event_name_id,c.seq,c.payload,n.name FROM clickman_chunks c JOIN clickman_event_names n ON n.project_id=c.project_id AND n.id=c.event_name_id WHERE c.project_id=$1 AND c.day BETWEEN $2 AND $3 AND (c.day,c.event_name_id,c.seq)>($4,$5,$6) ORDER BY c.day,c.event_name_id,c.seq LIMIT 64",vec![project.into(),from.into(),today.into(),cursor.0.into(),cursor.1.into(),cursor.2.into()])).await?;
            let count = page.len();
            for chunk in page {
                let day: NaiveDate = chunk.try_get("", "day")?;
                let name: String = chunk.try_get("", "name")?;
                cursor = (
                    day,
                    chunk.try_get("", "event_name_id")?,
                    chunk.try_get("", "seq")?,
                );
                let payload: Vec<u8> = chunk.try_get("", "payload")?;
                for row in decode(&payload)? {
                    if day >= today - Duration::days(29) {
                        let tally = totals.entry(name.clone()).or_default();
                        tally.0 += 1;
                        if Some(row.actor) != anonymous {
                            tally.1.insert(row.actor);
                        }
                    }
                    if Some(row.actor) == anonymous {
                        continue;
                    }
                    for (i, funnel) in funnels
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| f.event_names().contains(&name))
                    {
                        let _ = funnel;
                        journeys[i].push(Occurrence {
                            at: day
                                .and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_utc()
                                .timestamp_millis()
                                + row.millisecond,
                            actor: row.actor,
                            event: name.clone(),
                            fields: row
                                .fields
                                .iter()
                                .filter_map(|(id, v)| keys.get(id).map(|k| (k.clone(), v.clone())))
                                .collect(),
                        });
                    }
                }
            }
            if count < 64 {
                break;
            }
        }
        let mut totals:Vec<_>=totals.into_iter().map(|(event,(events,actors))|json!({"event":event,"events":events,"actors":actors.len()})).collect();
        totals.sort_by(|a, b| {
            b["events"]
                .as_u64()
                .cmp(&a["events"].as_u64())
                .then(a["event"].as_str().cmp(&b["event"].as_str()))
        });
        save(&tx, project, "events", json!({"events":totals}), now).await?;
        for (funnel, journey) in funnels.iter().zip(journeys) {
            save(
                &tx,
                project,
                &format!("funnel:{}", funnel.key),
                funnel.compute(journey, now),
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
