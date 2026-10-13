use crate::{Error, MetadataStore, Result, WriteIdentity, lock_project, sql};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, TransactionTrait};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) async fn canonical_ids(
    db: &impl ConnectionTrait,
    project: Uuid,
) -> Result<BTreeMap<i64, i64>> {
    db.query_all_raw(sql("SELECT a.id,c.id AS canonical FROM clickman_actors a JOIN clickman_actors c ON c.project_id=a.project_id AND c.id=COALESCE(a.merged_into_id,a.id) WHERE a.project_id=$1 AND a.erased_at IS NULL AND c.erased_at IS NULL AND c.external_id<>'*'",vec![project.into()])).await?.into_iter().map(|r|Ok((r.try_get("","id")?,r.try_get("","canonical")?))).collect()
}
pub(crate) async fn propagate_cohorts(db: &impl ConnectionTrait, project: Uuid) -> Result<()> {
    db.execute_raw(sql("UPDATE clickman_actors c SET first_seen_on=CASE WHEN c.has_events THEN LEAST(c.first_seen_on,a.first_seen_on) ELSE a.first_seen_on END,has_events=true FROM (SELECT merged_into_id,min(first_seen_on) AS first_seen_on FROM clickman_actors WHERE project_id=$1 AND merged_into_id IS NOT NULL AND erased_at IS NULL AND has_events GROUP BY merged_into_id) a WHERE c.project_id=$1 AND c.id=a.merged_into_id AND (NOT c.has_events OR c.first_seen_on>a.first_seen_on)",vec![project.into()])).await?;
    Ok(())
}
pub(crate) async fn identity_family(
    db: &impl ConnectionTrait,
    project: Uuid,
    external: &str,
) -> Result<Vec<String>> {
    let rows=db.query_all_raw(sql("SELECT a.external_id FROM clickman_actors a WHERE a.project_id=$1 AND a.erased_at IS NULL AND COALESCE(a.merged_into_id,a.id)=(SELECT COALESCE(b.merged_into_id,b.id) FROM clickman_actors b WHERE b.project_id=$1 AND b.external_id=$2)",vec![project.into(),external.into()])).await?;
    let mut names = rows
        .into_iter()
        .map(|r| r.try_get("", "external_id"))
        .collect::<std::result::Result<Vec<String>, _>>()?;
    if !names.iter().any(|n| n == external) {
        names.push(external.into());
    }
    Ok(names)
}
impl MetadataStore {
    /// Links only a browser-generated anonymous visitor to one opaque account.
    /// Original event IDs and physical rows remain intact; report readers map
    /// both actors to the same canonical identity. Linking is idempotent and
    /// cannot reassign an existing visitor to a different account.
    pub async fn link_identity(
        &self,
        identity: &WriteIdentity,
        anonymous: &str,
        external: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if anonymous
            .strip_prefix("anon:")
            .and_then(|s| Uuid::parse_str(s).ok())
            .is_none()
            || external.trim().is_empty()
            || external.chars().count() > 256
            || external.chars().any(char::is_control)
            || external == "*"
            || external.starts_with("anon:")
        {
            return Err(Error::Invalid("invalid identity link".into()));
        }
        let tx = self.db.begin().await?;
        lock_project(&tx, identity.project_id).await?;
        if tx.query_one_raw(sql("SELECT id FROM clickman_write_keys WHERE project_id=$1 AND id=$2 AND revoked_at IS NULL",vec![identity.project_id.into(),identity.key_id.into()])).await?.is_none(){return Err(Error::Invalid("write key revoked".into()));}
        for name in [anonymous, external] {
            tx.execute_raw(sql("INSERT INTO clickman_actors(project_id,external_id,first_seen_on,has_events) VALUES($1,$2,$3,false) ON CONFLICT(project_id,external_id) DO NOTHING",vec![identity.project_id.into(),name.into(),now.date_naive().into()])).await?;
        }
        let user: i64 = tx
            .query_one_raw(sql(
                "SELECT id FROM clickman_actors WHERE project_id=$1 AND external_id=$2",
                vec![identity.project_id.into(), external.into()],
            ))
            .await?
            .expect("canonical actor")
            .try_get("", "id")?;
        let existing: Option<i64> = tx
            .query_one_raw(sql(
                "SELECT merged_into_id FROM clickman_actors WHERE project_id=$1 AND external_id=$2",
                vec![identity.project_id.into(), anonymous.into()],
            ))
            .await?
            .expect("visitor actor")
            .try_get("", "merged_into_id")?;
        if existing.is_some_and(|id| id != user) {
            return Err(Error::Invalid(
                "visitor is already linked; reset identity before switching accounts".into(),
            ));
        }
        tx.execute_raw(sql(
            "UPDATE clickman_actors SET merged_into_id=$3 WHERE project_id=$1 AND external_id=$2",
            vec![identity.project_id.into(), anonymous.into(), user.into()],
        ))
        .await?;
        propagate_cohorts(&tx, identity.project_id).await?;
        // Inherit active membership before the next recomputation; identifying
        // an already-present visitor must not generate a second entry event.
        tx.execute_raw(sql("INSERT INTO clickman_segment_members(project_id,segment_id,actor_id,present,entries,revision,entered_at,evaluated_at) SELECT m.project_id,m.segment_id,$2,bool_or(m.present),max(m.entries),m.revision,min(m.entered_at),max(m.evaluated_at) FROM clickman_segment_members m JOIN clickman_segments s ON s.project_id=m.project_id AND s.id=m.segment_id AND s.revision=m.revision WHERE m.project_id=$1 AND m.actor_id IN (SELECT a.id FROM clickman_actors a WHERE a.project_id=$1 AND COALESCE(a.merged_into_id,a.id)=$2) GROUP BY m.project_id,m.segment_id,m.revision ON CONFLICT(project_id,segment_id,actor_id) DO UPDATE SET present=EXCLUDED.present,entries=EXCLUDED.entries,revision=EXCLUDED.revision,entered_at=EXCLUDED.entered_at,evaluated_at=EXCLUDED.evaluated_at",vec![identity.project_id.into(),user.into()])).await?;
        tx.execute_raw(sql("UPDATE clickman_segment_members m SET present=false WHERE m.project_id=$1 AND m.actor_id IN (SELECT a.id FROM clickman_actors a WHERE a.project_id=$1 AND a.merged_into_id=$2)",vec![identity.project_id.into(),user.into()])).await?;
        tx.execute_raw(sql(
            "DELETE FROM clickman_reports WHERE project_id=$1",
            vec![identity.project_id.into()],
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
