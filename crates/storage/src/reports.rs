use crate::{MetadataStore, Result, sql};
use chrono::{DateTime, Utc};
use sea_orm::ConnectionTrait;
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
        self.db.query_all_raw(sql("SELECT key,result,computed_at FROM clickman_reports WHERE project_id=$1 ORDER BY key",vec![project.into()])).await?.into_iter().map(|r|Ok(Report{key:r.try_get("","key")?,result:r.try_get("","result")?,computed_at:r.try_get("","computed_at")?})).collect()
    }
}
