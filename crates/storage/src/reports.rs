use crate::{Error, MetadataStore, Result, entities::report};
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub key: String,
    pub result: Value,
    pub computed_at: DateTime<Utc>,
}

impl MetadataStore {
    pub async fn reports(&self, project: Uuid) -> Result<Vec<Report>> {
        self.filtered_reports(project, None).await
    }

    /// None returns all snapshots; an explicit empty key list returns none.
    /// Filter in the database before transferring potentially large results.
    pub async fn filtered_reports(
        &self,
        project: Uuid,
        keys: Option<&[String]>,
    ) -> Result<Vec<Report>> {
        let mut query = report::Entity::find().filter(report::Column::ProjectId.eq(project));
        if let Some(keys) = keys {
            if keys.len() > 32
                || keys.iter().any(|key| {
                    key.is_empty() || key.len() > 200 || key.chars().any(char::is_control)
                })
            {
                return Err(Error::Invalid(
                    "choose up to 32 report keys of 1..200 bytes without control characters".into(),
                ));
            }
            if keys.is_empty() {
                return Ok(Vec::new());
            }
            query = query.filter(report::Column::Key.is_in(keys.iter().cloned()));
        }
        Ok(query
            .order_by_asc(report::Column::Key)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|record| Report {
                key: record.key,
                result: record.result,
                computed_at: record.computed_at,
            })
            .collect())
    }
}

pub(crate) async fn save(
    db: &impl ConnectionTrait,
    project: Uuid,
    key: &str,
    result: Value,
    now: DateTime<Utc>,
) -> Result<()> {
    report::Entity::insert(report::ActiveModel {
        project_id: Set(project),
        key: Set(key.into()),
        result: Set(result),
        computed_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([report::Column::ProjectId, report::Column::Key])
            .update_columns([report::Column::Result, report::Column::ComputedAt])
            .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}
