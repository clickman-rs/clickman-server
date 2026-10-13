use crate::entities::{access_key, write_key};
use crate::{Access, Error, MetadataStore, ProjectSettings, Result, Role, digest, new_token};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
/// The token is returned exactly once, separately from project/key records.
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

fn role(value: &str) -> Result<Role> {
    match value {
        "reader" => Ok(Role::Reader),
        "admin" => Ok(Role::Admin),
        _ => Err(Error::Invalid("invalid stored access role".into())),
    }
}

impl MetadataStore {
    pub async fn issue_write_key(&self, project: Uuid, source: &str) -> Result<IssuedKey> {
        crate::management::label(source)?;
        let id = Uuid::new_v4();
        let token = new_token("cmw");
        write_key::ActiveModel {
            id: Set(id),
            project_id: Set(project),
            source: Set(source.into()),
            digest: Set(digest(&token)),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;
        Ok(IssuedKey { id, token })
    }

    pub async fn write_keys(&self, project: Uuid) -> Result<Vec<WriteKey>> {
        Ok(write_key::Entity::find()
            .filter(write_key::Column::ProjectId.eq(project))
            .order_by_asc(write_key::Column::CreatedAt)
            .order_by_asc(write_key::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|key| WriteKey {
                id: key.id,
                source: key.source,
                revoked_at: key.revoked_at,
            })
            .collect())
    }

    pub async fn revoke_write_key(&self, project: Uuid, id: Uuid) -> Result<bool> {
        Ok(write_key::Entity::update_many()
            .col_expr(write_key::Column::RevokedAt, Expr::current_timestamp())
            .filter(write_key::Column::ProjectId.eq(project))
            .filter(write_key::Column::Id.eq(id))
            .filter(write_key::Column::RevokedAt.is_null())
            .exec(&self.db)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn write_identity(&self, token: &str) -> Result<Option<WriteIdentity>> {
        let Some((key, project)) = write_key::Entity::find()
            .filter(write_key::Column::Digest.eq(digest(token)))
            .filter(write_key::Column::RevokedAt.is_null())
            .find_also_related(crate::entities::project::Entity)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let project =
            project.ok_or_else(|| Error::Invalid("write key project not found".into()))?;
        Ok(Some(WriteIdentity {
            key_id: key.id,
            project_id: key.project_id,
            source: key.source,
            settings: serde_json::from_value(project.settings)
                .map_err(|error| Error::Invalid(error.to_string()))?,
        }))
    }

    pub async fn issue_access_key(
        &self,
        project: Uuid,
        name: &str,
        role: Role,
    ) -> Result<IssuedKey> {
        crate::management::label(name)?;
        let id = Uuid::new_v4();
        let token = new_token("cma");
        access_key::ActiveModel {
            id: Set(id),
            project_id: Set(project),
            name: Set(name.into()),
            role: Set(role.as_str().into()),
            digest: Set(digest(&token)),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;
        Ok(IssuedKey { id, token })
    }

    pub async fn access_keys(&self, project: Uuid) -> Result<Vec<AccessKey>> {
        access_key::Entity::find()
            .filter(access_key::Column::ProjectId.eq(project))
            .order_by_asc(access_key::Column::CreatedAt)
            .order_by_asc(access_key::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|key| {
                Ok(AccessKey {
                    id: key.id,
                    name: key.name,
                    role: role(&key.role)?,
                    revoked_at: key.revoked_at,
                })
            })
            .collect()
    }

    pub async fn revoke_access_key(&self, project: Uuid, id: Uuid) -> Result<bool> {
        Ok(access_key::Entity::update_many()
            .col_expr(access_key::Column::RevokedAt, Expr::current_timestamp())
            .filter(access_key::Column::ProjectId.eq(project))
            .filter(access_key::Column::Id.eq(id))
            .filter(access_key::Column::RevokedAt.is_null())
            .exec(&self.db)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn access_identity(&self, token: &str) -> Result<Option<Access>> {
        access_key::Entity::find()
            .filter(access_key::Column::Digest.eq(digest(token)))
            .filter(access_key::Column::RevokedAt.is_null())
            .one(&self.db)
            .await?
            .map(|key| Ok(Access::project(key.project_id, role(&key.role)?)))
            .transpose()
    }
}
