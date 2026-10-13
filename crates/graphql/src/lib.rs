use async_graphql::{
    ComplexObject, Context, EmptySubscription, Enum, ErrorExtensions, ID, Json, Object, Schema,
    SimpleObject,
    connection::{Connection, Edge},
};
use chrono::{DateTime, Utc};
use clickman_storage::{Access, Error as StorageError, ProjectSettings, Role, Storage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub type ClickmanSchema = Schema<Query, Mutation, EmptySubscription>;

pub fn sdl() -> String {
    Schema::build(Query, Mutation, EmptySubscription)
        .finish()
        .sdl()
}

pub fn schema(storage: Storage) -> ClickmanSchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .data(storage)
        .limit_depth(10)
        .limit_complexity(1000)
        .finish()
}

fn access<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Access> {
    ctx.data_opt::<Access>()
        .ok_or_else(|| denied("authentication required"))
}
fn denied(message: &str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, e| e.set("code", "FORBIDDEN"))
}
fn read(ctx: &Context<'_>, project: Uuid) -> async_graphql::Result<()> {
    if access(ctx)?.can_read(project) {
        Ok(())
    } else {
        Err(denied("project access denied"))
    }
}
fn admin(ctx: &Context<'_>, project: Uuid) -> async_graphql::Result<()> {
    if access(ctx)?.can_admin(project) {
        Ok(())
    } else {
        Err(denied("project administration denied"))
    }
}
fn error(error: StorageError) -> async_graphql::Error {
    match error {
        StorageError::Invalid(message) => {
            async_graphql::Error::new(message).extend_with(|_, e| e.set("code", "BAD_USER_INPUT"))
        }
        error => {
            tracing::error!(%error,"GraphQL storage operation failed");
            async_graphql::Error::new("storage operation failed")
                .extend_with(|_, e| e.set("code", "INTERNAL"))
        }
    }
}
fn store<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Storage> {
    ctx.data::<Storage>()
}

fn record_id(project: Uuid, key: &str) -> ID {
    format!("{project}-{:x}", Sha256::digest(key.as_bytes())).into()
}

pub struct Query;
#[Object]
impl Query {
    async fn viewer(&self, ctx: &Context<'_>) -> async_graphql::Result<Viewer> {
        Ok(Viewer {
            can_create_projects: access(ctx)?.is_root(),
        })
    }
    async fn projects(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Connection<String, Project>> {
        let access = access(ctx)?;
        let mut connection = Connection::new(false, false);
        connection.edges = store(ctx)?
            .projects()
            .await
            .map_err(error)?
            .into_iter()
            .filter(|p| access.can_read(p.id))
            .map(|p| Edge::new(p.id.to_string(), Project::from(p)))
            .collect();
        Ok(connection)
    }
    async fn project(&self, ctx: &Context<'_>, id: Uuid) -> async_graphql::Result<Project> {
        read(ctx, id)?;
        Ok(Project::from(store(ctx)?.project(id).await.map_err(error)?))
    }
}

#[derive(SimpleObject)]
pub struct Viewer {
    pub can_create_projects: bool,
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Project {
    pub id: ID,
    pub slug: String,
    pub name: String,
    pub settings: Json<Value>,
    pub created_at: DateTime<Utc>,
    #[graphql(skip)]
    uuid: Uuid,
}
impl From<clickman_storage::Project> for Project {
    fn from(p: clickman_storage::Project) -> Self {
        Self {
            id: p.id.to_string().into(),
            slug: p.slug,
            name: p.name,
            settings: Json(p.settings),
            created_at: p.created_at,
            uuid: p.id,
        }
    }
}
#[ComplexObject]
impl Project {
    async fn can_admin(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        Ok(access(ctx)?.can_admin(self.uuid))
    }
    async fn reports(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Report>> {
        read(ctx, self.uuid)?;
        Ok(store(ctx)?
            .reports(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(|r| Report {
                id: record_id(self.uuid, &r.key),
                key: r.key,
                result: Json(r.result),
                computed_at: r.computed_at,
            })
            .collect())
    }
    async fn funnels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Funnel>> {
        read(ctx, self.uuid)?;
        Ok(store(ctx)?
            .funnels(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(|f| Funnel {
                id: record_id(self.uuid, &f.key),
                key: f.key.clone(),
                name: f.name.clone(),
                definition: Json(serde_json::to_value(f).unwrap()),
            })
            .collect())
    }
    async fn write_keys(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<WriteKey>> {
        admin(ctx, self.uuid)?;
        Ok(store(ctx)?
            .write_keys(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(|k| WriteKey {
                id: k.id.to_string().into(),
                source: k.source,
                revoked_at: k.revoked_at,
            })
            .collect())
    }
    async fn access_keys(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<AccessKey>> {
        admin(ctx, self.uuid)?;
        Ok(store(ctx)?
            .access_keys(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(|k| AccessKey {
                id: k.id.to_string().into(),
                name: k.name,
                role: k.role.into(),
                revoked_at: k.revoked_at,
            })
            .collect())
    }
    async fn destinations(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Destination>> {
        admin(ctx, self.uuid)?;
        Ok(store(ctx)?
            .destinations(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(|d| Destination {
                id: d.id.to_string().into(),
                name: d.name,
                kind: d.kind,
                config: Json(d.config),
            })
            .collect())
    }
}

#[derive(SimpleObject)]
pub struct Report {
    pub id: ID,
    pub key: String,
    pub result: Json<Value>,
    pub computed_at: DateTime<Utc>,
}
#[derive(SimpleObject)]
pub struct Funnel {
    pub id: ID,
    pub key: String,
    pub name: String,
    pub definition: Json<Value>,
}
#[derive(SimpleObject)]
pub struct WriteKey {
    pub id: ID,
    pub source: String,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(SimpleObject)]
pub struct AccessKey {
    pub id: ID,
    pub name: String,
    pub role: ProjectRole,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(SimpleObject)]
pub struct IssuedKey {
    pub id: ID,
    pub token: String,
}
impl From<clickman_storage::IssuedKey> for IssuedKey {
    fn from(k: clickman_storage::IssuedKey) -> Self {
        Self {
            id: k.id.to_string().into(),
            token: k.token,
        }
    }
}
#[derive(SimpleObject)]
pub struct Destination {
    pub id: ID,
    pub name: String,
    pub kind: String,
    pub config: Json<Value>,
}
#[derive(Enum, Clone, Copy, Eq, PartialEq)]
pub enum ProjectRole {
    Reader,
    Admin,
}
impl From<Role> for ProjectRole {
    fn from(r: Role) -> Self {
        match r {
            Role::Reader => Self::Reader,
            Role::Admin => Self::Admin,
        }
    }
}
impl From<ProjectRole> for Role {
    fn from(r: ProjectRole) -> Self {
        match r {
            ProjectRole::Reader => Self::Reader,
            ProjectRole::Admin => Self::Admin,
        }
    }
}

pub struct Mutation;
#[Object]
impl Mutation {
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        slug: String,
        name: String,
    ) -> async_graphql::Result<Project> {
        if !access(ctx)?.is_root() {
            return Err(denied(
                "only the instance administrator can create projects",
            ));
        }
        Ok(store(ctx)?
            .create_project(&slug, &name)
            .await
            .map_err(error)?
            .into())
    }
    async fn update_settings(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        settings: Json<Value>,
    ) -> async_graphql::Result<Project> {
        admin(ctx, project_id)?;
        let settings: ProjectSettings = serde_json::from_value(settings.0)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(store(ctx)?
            .update_settings(project_id, settings)
            .await
            .map_err(error)?
            .into())
    }
    async fn issue_write_key(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        source: String,
    ) -> async_graphql::Result<IssuedKey> {
        admin(ctx, project_id)?;
        Ok(store(ctx)?
            .issue_write_key(project_id, &source)
            .await
            .map_err(error)?
            .into())
    }
    async fn revoke_write_key(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .revoke_write_key(project_id, id)
            .await
            .map_err(error)
    }
    async fn issue_access_key(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        name: String,
        role: ProjectRole,
    ) -> async_graphql::Result<IssuedKey> {
        admin(ctx, project_id)?;
        Ok(store(ctx)?
            .issue_access_key(project_id, &name, role.into())
            .await
            .map_err(error)?
            .into())
    }
    async fn revoke_access_key(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .revoke_access_key(project_id, id)
            .await
            .map_err(error)
    }
    async fn save_funnel(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        definition: Json<Value>,
    ) -> async_graphql::Result<Funnel> {
        admin(ctx, project_id)?;
        let f = store(ctx)?
            .save_funnel(project_id, definition.0)
            .await
            .map_err(error)?;
        Ok(Funnel {
            id: record_id(project_id, &f.key),
            key: f.key.clone(),
            name: f.name.clone(),
            definition: Json(serde_json::to_value(f).unwrap()),
        })
    }
    async fn delete_funnel(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        key: String,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .delete_funnel(project_id, &key)
            .await
            .map_err(error)
    }
    async fn erase_actor(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        external_id: String,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .erase(project_id, &external_id, Utc::now())
            .await
            .map_err(error)
    }
    async fn refresh_analytics(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        let storage = store(ctx)?;
        let now = Utc::now();
        storage.rotate(project_id, now).await.map_err(error)?;
        storage.prune(project_id, now).await.map_err(error)?;
        storage
            .refresh_reports(project_id, now)
            .await
            .map_err(error)?;
        Ok(true)
    }
    async fn save_mixpanel_destination(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        name: String,
        config: Json<Value>,
    ) -> async_graphql::Result<Destination> {
        admin(ctx, project_id)?;
        let validated = clickman_destinations::MixpanelConfig::parse(project_id, config.0)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let d = store(ctx)?
            .save_destination(
                project_id,
                &name,
                "mixpanel",
                serde_json::to_value(validated).unwrap(),
            )
            .await
            .map_err(error)?;
        Ok(Destination {
            id: d.id.to_string().into(),
            name: d.name,
            kind: d.kind,
            config: Json(d.config),
        })
    }
    async fn delete_destination(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .delete_destination(project_id, id)
            .await
            .map_err(error)
    }
}
