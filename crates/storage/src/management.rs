use crate::entities::{funnel as funnel_record, project, report};
use crate::{Access, Error, MetadataStore, ProjectSettings, Result, sql};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, sea_query::OnConflict,
};
use serde_json::Value;
use uuid::Uuid;

pub type Project = project::Model;

pub(crate) fn label(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > 200 {
        Err(Error::Invalid("name must contain 1..200 bytes".into()))
    } else {
        Ok(())
    }
}

impl MetadataStore {
    pub async fn projects(&self) -> Result<Vec<Project>> {
        Ok(project::Entity::find()
            .order_by_asc(project::Column::CreatedAt)
            .order_by_asc(project::Column::Id)
            .all(&self.db)
            .await?)
    }
    /// Apply explicit host grants in PostgreSQL rather than loading every
    /// organization's metadata and filtering it in the application process.
    pub async fn accessible_projects(&self, access: &Access) -> Result<Vec<Project>> {
        if access.is_root() {
            return self.projects().await;
        }
        let ids: Vec<_> = access.project_ids().collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(project::Entity::find()
            .filter(project::Column::Id.is_in(ids))
            .order_by_asc(project::Column::CreatedAt)
            .order_by_asc(project::Column::Id)
            .all(&self.db)
            .await?)
    }
    pub async fn project(&self, id: Uuid) -> Result<Project> {
        project::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::Invalid("project not found".into()))
    }
    pub async fn create_project(&self, slug: &str, name: &str) -> Result<Project> {
        self.create_project_in_namespace("", slug, name).await
    }
    /// Hosts supply an opaque scope for per-organization slug uniqueness. The
    /// engine remains unaware of organization accounts or membership tables.
    pub async fn create_project_in_namespace(
        &self,
        namespace: &str,
        slug: &str,
        name: &str,
    ) -> Result<Project> {
        if namespace.len() > 128 {
            return Err(Error::Invalid("project namespace is too long".into()));
        }
        label(name)?;
        if slug.is_empty()
            || slug.len() > 64
            || !slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(Error::Invalid(
                "slug must be lowercase letters, digits, hyphens or underscores (1..64 bytes)"
                    .into(),
            ));
        }
        Ok(project::ActiveModel {
            id: Set(Uuid::new_v4()),
            slug: Set(slug.into()),
            namespace: Set(namespace.into()),
            name: Set(name.into()),
            settings: Set(
                serde_json::to_value(ProjectSettings::default()).expect("serializable settings")
            ),
            created_at: Set(Utc::now()),
        }
        .insert(&self.db)
        .await?)
    }
    pub async fn update_settings(&self, id: Uuid, settings: ProjectSettings) -> Result<Project> {
        settings.validate()?;
        let tx = self.db.begin().await?;
        crate::lock_project(&tx, id).await?;
        let saved = project::ActiveModel {
            id: Set(id),
            settings: Set(serde_json::to_value(settings).expect("serializable settings")),
            ..Default::default()
        }
        .update(&tx)
        .await?;
        tx.commit().await?;
        Ok(saved)
    }
    pub async fn funnels(&self, project: Uuid) -> Result<Vec<clickman_analytics::Funnel>> {
        load_funnels(&self.db, project).await
    }
    pub async fn save_funnel(
        &self,
        project: Uuid,
        definition: Value,
    ) -> Result<clickman_analytics::Funnel> {
        let funnel = clickman_analytics::Funnel::parse(definition.clone())
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let tx = self.db.begin().await?;
        crate::lock_project(&tx, project).await?;
        let changed = if let Some(previous) =
            funnel_record::Entity::find_by_id((project, funnel.key.clone()))
                .one(&tx)
                .await?
        {
            let old = clickman_analytics::Funnel::parse(previous.definition)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            serde_json::to_value(old.steps).expect("funnel steps")
                != serde_json::to_value(&funnel.steps).expect("funnel steps")
                || old.window != funnel.window
                || old.range != funnel.range
        } else {
            false
        };
        // JSON rule references are guarded while the project lock is held.
        let removed_step = tx
            .query_one_raw(sql(
                "SELECT id FROM clickman_segments
             WHERE project_id=$1
               AND definition->'rule'->>'kind'='funnel'
               AND definition->'rule'->>'funnel_key'=$2
               AND (definition->'rule'->>'at_step')::int>$3
             LIMIT 1",
                vec![
                    project.into(),
                    funnel.key.clone().into(),
                    (funnel.steps.len() as i32).into(),
                ],
            ))
            .await?
            .is_some();
        if removed_step {
            return Err(Error::Invalid(
                "a segment references a step removed by this funnel change".into(),
            ));
        }
        funnel_record::Entity::insert(funnel_record::ActiveModel {
            project_id: Set(project),
            key: Set(funnel.key.clone()),
            definition: Set(definition),
        })
        .on_conflict(
            OnConflict::columns([funnel_record::Column::ProjectId, funnel_record::Column::Key])
                .update_column(funnel_record::Column::Definition)
                .to_owned(),
        )
        .exec(&tx)
        .await?;
        if changed {
            tx.execute_raw(sql(
                "UPDATE clickman_segments
                 SET revision=revision+1,updated_at=now(),evaluated_at=NULL
                 WHERE project_id=$1
                   AND definition->'rule'->>'kind'='funnel'
                   AND definition->'rule'->>'funnel_key'=$2",
                vec![project.into(), funnel.key.clone().into()],
            ))
            .await?;
            tx.execute_raw(sql(
                "UPDATE clickman_webhook_deliveries d SET status='canceled'
                 FROM clickman_webhooks w
                 JOIN clickman_segments s ON s.project_id=w.project_id AND s.id=w.segment_id
                 WHERE d.project_id=$1 AND w.project_id=d.project_id AND d.webhook_id=w.id
                   AND d.status='pending'
                   AND s.definition->'rule'->>'kind'='funnel'
                   AND s.definition->'rule'->>'funnel_key'=$2",
                vec![project.into(), funnel.key.clone().into()],
            ))
            .await?;
        }
        delete_funnel_report(&tx, project, &funnel.key).await?;
        tx.commit().await?;
        Ok(funnel)
    }
    pub async fn delete_funnel(&self, project: Uuid, key: &str) -> Result<bool> {
        let tx = self.db.begin().await?;
        crate::lock_project(&tx, project).await?;
        let referenced = tx
            .query_one_raw(sql(
                "SELECT id FROM clickman_segments
             WHERE project_id=$1
               AND definition->'rule'->>'kind'='funnel'
               AND definition->'rule'->>'funnel_key'=$2
             LIMIT 1",
                vec![project.into(), key.into()],
            ))
            .await?
            .is_some();
        if referenced {
            return Err(Error::Invalid(
                "delete the segments referencing this funnel first".into(),
            ));
        }
        let changed = funnel_record::Entity::delete_many()
            .filter(funnel_record::Column::ProjectId.eq(project))
            .filter(funnel_record::Column::Key.eq(key))
            .exec(&tx)
            .await?
            .rows_affected
            > 0;
        delete_funnel_report(&tx, project, key).await?;
        tx.commit().await?;
        Ok(changed)
    }
}

/// Reads through the caller's connection, including a held transaction. Remote
/// analytics report generation must not reacquire its metadata pool here.
pub(crate) async fn load_funnels(
    db: &impl ConnectionTrait,
    project: Uuid,
) -> Result<Vec<clickman_analytics::Funnel>> {
    funnel_record::Entity::find()
        .filter(funnel_record::Column::ProjectId.eq(project))
        .order_by_asc(funnel_record::Column::Key)
        .all(db)
        .await?
        .into_iter()
        .map(|record| {
            clickman_analytics::Funnel::parse(record.definition)
                .map_err(|error| Error::Invalid(error.to_string()))
        })
        .collect()
}

async fn delete_funnel_report(db: &impl ConnectionTrait, project: Uuid, key: &str) -> Result<()> {
    report::Entity::delete_many()
        .filter(report::Column::ProjectId.eq(project))
        .filter(report::Column::Key.eq(format!("funnel:{key}")))
        .exec(db)
        .await?;
    Ok(())
}
