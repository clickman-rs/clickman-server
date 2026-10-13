use crate::entities::project;
use crate::{Error, ProjectSettings, Result, Role, Storage, digest, new_token, sql};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub type Project = project::Model;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteKey {
    pub id: Uuid,
    pub source: String,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKey {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
    pub revoked_at: Option<DateTime<Utc>>,
}
/// The token is returned exactly once. Do not log or serialize this as a project record.
pub struct IssuedKey {
    pub id: Uuid,
    pub token: String,
}
pub struct WriteIdentity {
    pub key_id: Uuid,
    pub project_id: Uuid,
    pub source: String,
    pub settings: ProjectSettings,
}

fn label(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > 200 {
        Err(Error::Invalid("name must contain 1..200 bytes".into()))
    } else {
        Ok(())
    }
}

impl Storage {
    pub async fn projects(&self) -> Result<Vec<Project>> {
        Ok(project::Entity::find().all(&self.db).await?)
    }
    pub async fn project(&self, id: Uuid) -> Result<Project> {
        project::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::Invalid("project not found".into()))
    }
    pub async fn create_project(&self, slug: &str, name: &str) -> Result<Project> {
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
        tx.execute_raw(sql(
            "UPDATE clickman_projects SET settings=$2 WHERE id=$1",
            vec![id.into(), serde_json::to_value(settings).unwrap().into()],
        ))
        .await?;
        tx.commit().await?;
        self.project(id).await
    }
    pub async fn issue_write_key(&self, project: Uuid, source: &str) -> Result<IssuedKey> {
        label(source)?;
        let id = Uuid::new_v4();
        let token = new_token("cmw");
        self.db
            .execute_raw(sql(
                "INSERT INTO clickman_write_keys(id,project_id,source,digest) VALUES($1,$2,$3,$4)",
                vec![
                    id.into(),
                    project.into(),
                    source.into(),
                    digest(&token).into(),
                ],
            ))
            .await?;
        Ok(IssuedKey { id, token })
    }
    pub async fn write_keys(&self, project: Uuid) -> Result<Vec<WriteKey>> {
        self.db.query_all_raw(sql("SELECT id,source,revoked_at FROM clickman_write_keys WHERE project_id=$1 ORDER BY created_at,id",vec![project.into()])).await?.into_iter().map(|r| Ok(WriteKey{id:r.try_get("","id")?,source:r.try_get("","source")?,revoked_at:r.try_get("","revoked_at")?})).collect()
    }
    pub async fn revoke_write_key(&self, project: Uuid, id: Uuid) -> Result<bool> {
        Ok(self.db.execute_raw(sql("UPDATE clickman_write_keys SET revoked_at=now() WHERE project_id=$1 AND id=$2 AND revoked_at IS NULL",vec![project.into(),id.into()])).await?.rows_affected()>0)
    }
    pub async fn write_identity(&self, token: &str) -> Result<Option<WriteIdentity>> {
        let Some(r)=self.db.query_one_raw(sql("SELECT k.id,k.project_id,k.source,p.settings FROM clickman_write_keys k JOIN clickman_projects p ON p.id=k.project_id WHERE k.digest=$1 AND k.revoked_at IS NULL",vec![digest(token).into()])).await? else { return Ok(None) };
        let settings: Value = r.try_get("", "settings")?;
        Ok(Some(WriteIdentity {
            key_id: r.try_get("", "id")?,
            project_id: r.try_get("", "project_id")?,
            source: r.try_get("", "source")?,
            settings: serde_json::from_value(settings)
                .map_err(|e| Error::Invalid(e.to_string()))?,
        }))
    }
    pub async fn issue_access_key(
        &self,
        project: Uuid,
        name: &str,
        role: Role,
    ) -> Result<IssuedKey> {
        label(name)?;
        let id = Uuid::new_v4();
        let token = new_token("cma");
        self.db.execute_raw(sql("INSERT INTO clickman_access_keys(id,project_id,name,role,digest) VALUES($1,$2,$3,$4,$5)",vec![id.into(),project.into(),name.into(),role.as_str().into(),digest(&token).into()])).await?;
        Ok(IssuedKey { id, token })
    }
    pub async fn access_keys(&self, project: Uuid) -> Result<Vec<AccessKey>> {
        self.db.query_all_raw(sql("SELECT id,name,role,revoked_at FROM clickman_access_keys WHERE project_id=$1 ORDER BY created_at,id",vec![project.into()])).await?.into_iter().map(|r| Ok(AccessKey{id:r.try_get("","id")?,name:r.try_get("","name")?,role:if r.try_get::<String>("","role")?=="admin" {Role::Admin}else{Role::Reader},revoked_at:r.try_get("","revoked_at")?})).collect()
    }
    pub async fn revoke_access_key(&self, project: Uuid, id: Uuid) -> Result<bool> {
        Ok(self.db.execute_raw(sql("UPDATE clickman_access_keys SET revoked_at=now() WHERE project_id=$1 AND id=$2 AND revoked_at IS NULL",vec![project.into(),id.into()])).await?.rows_affected()>0)
    }
    pub async fn access_identity(&self, token: &str) -> Result<Option<crate::Access>> {
        let Some(r)=self.db.query_one_raw(sql("SELECT project_id,role FROM clickman_access_keys WHERE digest=$1 AND revoked_at IS NULL",vec![digest(token).into()])).await? else { return Ok(None) };
        Ok(Some(crate::Access::project(
            r.try_get("", "project_id")?,
            if r.try_get::<String>("", "role")? == "admin" {
                Role::Admin
            } else {
                Role::Reader
            },
        )))
    }
    pub async fn funnels(&self, project: Uuid) -> Result<Vec<clickman_analytics::Funnel>> {
        self.db
            .query_all_raw(sql(
                "SELECT definition FROM clickman_funnels WHERE project_id=$1 ORDER BY key",
                vec![project.into()],
            ))
            .await?
            .into_iter()
            .map(|r| {
                clickman_analytics::Funnel::parse(r.try_get("", "definition")?)
                    .map_err(|e| Error::Invalid(e.to_string()))
            })
            .collect()
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
        tx.execute_raw(sql("INSERT INTO clickman_funnels(project_id,key,definition) VALUES($1,$2,$3) ON CONFLICT(project_id,key) DO UPDATE SET definition=EXCLUDED.definition",vec![project.into(),funnel.key.clone().into(),definition.into()])).await?;
        tx.execute_raw(sql(
            "DELETE FROM clickman_reports WHERE project_id=$1 AND key=$2",
            vec![project.into(), format!("funnel:{}", funnel.key).into()],
        ))
        .await?;
        tx.commit().await?;
        Ok(funnel)
    }
    pub async fn delete_funnel(&self, project: Uuid, key: &str) -> Result<bool> {
        let tx = self.db.begin().await?;
        crate::lock_project(&tx, project).await?;
        let changed = tx
            .execute_raw(sql(
                "DELETE FROM clickman_funnels WHERE project_id=$1 AND key=$2",
                vec![project.into(), key.into()],
            ))
            .await?
            .rows_affected()
            > 0;
        tx.execute_raw(sql(
            "DELETE FROM clickman_reports WHERE project_id=$1 AND key=$2",
            vec![project.into(), format!("funnel:{key}").into()],
        ))
        .await?;
        tx.commit().await?;
        Ok(changed)
    }
}
