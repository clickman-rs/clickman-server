use async_graphql::{
    ComplexObject, Context, EmptySubscription, Enum, ErrorExtensions, ID, InputObject, Json,
    Object, Schema, SimpleObject,
    connection::{Connection, Edge},
};
use chrono::{DateTime, Utc};
use clickman_storage::{Access, Error as StorageError, ProjectSettings, Role, Storage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub type ClickmanSchema = Schema<Query, Mutation, EmptySubscription>;

/// Request-local capability supplied by an authorized host. Creating and
/// attaching the project share the host's transaction and authorization policy.
#[async_trait::async_trait]
pub trait ProjectCreator: Send + Sync {
    /// Permission on the newly returned object only. A host that attaches the
    /// creator as an administrator opts in; existing implementations retain a
    /// conservative reader role without changing request-wide Access.
    fn created_role(&self) -> Role {
        Role::Reader
    }
    async fn create(
        &self,
        storage: &Storage,
        slug: &str,
        name: &str,
    ) -> clickman_storage::Result<clickman_storage::Project>;
}
#[async_trait::async_trait]
pub trait IntegrationPolicy: Send + Sync {
    async fn validate(
        &self,
        project: Uuid,
        kind: &str,
        config: &Value,
    ) -> clickman_storage::Result<()>;
}

/// Optional request-local limits supplied by an authenticated embedding host.
/// Standalone installations retain their normal access-key and retention policy.
#[derive(Clone, Copy)]
pub struct ProjectPolicy {
    pub allow_access_keys: bool,
    pub max_retention_days: Option<u32>,
}

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
        StorageError::Capacity { .. } => async_graphql::Error::new("capacity exhausted")
            .extend_with(|_, e| e.set("code", "CAPACITY_EXHAUSTED")),
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
            can_create_projects: access(ctx)?.is_root()
                || ctx
                    .data_opt::<std::sync::Arc<dyn ProjectCreator>>()
                    .is_some(),
        })
    }
    async fn projects(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Connection<String, Project>> {
        let access = access(ctx)?;
        let mut connection = Connection::new(false, false);
        connection.edges = store(ctx)?
            .accessible_projects(access)
            .await
            .map_err(error)?
            .into_iter()
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
    #[graphql(skip)]
    created_role: Option<Role>,
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
            created_role: None,
        }
    }
}
impl Project {
    fn read(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        let permissions = access(ctx)?;
        if self.created_role.is_some() || permissions.can_read(self.uuid) {
            Ok(())
        } else {
            Err(denied("project access denied"))
        }
    }
    fn admin(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        let permissions = access(ctx)?;
        if self.created_role == Some(Role::Admin) || permissions.can_admin(self.uuid) {
            Ok(())
        } else {
            Err(denied("project administration denied"))
        }
    }
}
#[ComplexObject]
impl Project {
    async fn segments(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Segment>> {
        self.read(ctx)?;
        Ok(store(ctx)?
            .segments(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    async fn webhooks(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Webhook>> {
        self.admin(ctx)?;
        Ok(store(ctx)?
            .webhooks(self.uuid)
            .await
            .map_err(error)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    async fn webhook_logs(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 50)] limit: u64,
    ) -> async_graphql::Result<Vec<WebhookLog>> {
        self.admin(ctx)?;
        Ok(store(ctx)?
            .webhook_logs(self.uuid, limit)
            .await
            .map_err(error)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    async fn can_admin(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        Ok(access(ctx)?.can_admin(self.uuid) || self.created_role == Some(Role::Admin))
    }
    async fn reports(
        &self,
        ctx: &Context<'_>,
        keys: Option<Vec<String>>,
    ) -> async_graphql::Result<Vec<Report>> {
        self.read(ctx)?;
        Ok(store(ctx)?
            .filtered_reports(self.uuid, keys.as_deref())
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
        self.read(ctx)?;
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
        self.admin(ctx)?;
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
        self.admin(ctx)?;
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
        self.admin(ctx)?;
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
pub struct Segment {
    pub id: ID,
    pub key: String,
    pub name: String,
    pub definition: Json<Value>,
    pub enabled: bool,
    pub fire_existing: bool,
    pub revision: i32,
    pub members: i64,
    pub evaluated_at: Option<DateTime<Utc>>,
}
impl From<clickman_storage::SavedSegment> for Segment {
    fn from(s: clickman_storage::SavedSegment) -> Self {
        Self {
            id: s.id.to_string().into(),
            key: s.definition.key.clone(),
            name: s.definition.name.clone(),
            definition: Json(serde_json::to_value(s.definition).expect("segment definition")),
            enabled: s.enabled,
            fire_existing: s.fire_existing,
            revision: s.revision,
            members: s.members,
            evaluated_at: s.evaluated_at,
        }
    }
}
#[derive(SimpleObject)]
pub struct Webhook {
    pub id: ID,
    pub segment_id: ID,
    pub name: String,
    pub kind: String,
    pub config: Json<Value>,
    pub enabled: bool,
}
#[derive(InputObject)]
pub struct WebhookInput {
    pub segment_id: Uuid,
    pub name: String,
    pub kind: String,
    pub config: Json<Value>,
    #[graphql(default = true)]
    pub enabled: bool,
}
impl From<clickman_storage::Webhook> for Webhook {
    fn from(h: clickman_storage::Webhook) -> Self {
        Self {
            id: h.id.to_string().into(),
            segment_id: h.segment_id.to_string().into(),
            name: h.name,
            kind: h.kind,
            config: Json(h.config),
            enabled: h.enabled,
        }
    }
}
#[derive(SimpleObject)]
pub struct WebhookLog {
    pub id: ID,
    pub webhook_id: ID,
    pub entry_id: ID,
    pub status: String,
    pub attempts: i32,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub next_attempt_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
impl From<clickman_storage::WebhookLog> for WebhookLog {
    fn from(l: clickman_storage::WebhookLog) -> Self {
        Self {
            id: l.id.to_string().into(),
            webhook_id: l.webhook_id.to_string().into(),
            entry_id: l.entry_id.to_string().into(),
            status: l.status,
            attempts: l.attempts,
            last_status: l.last_status,
            last_error: l.last_error,
            next_attempt_at: l.next_attempt_at,
            completed_at: l.completed_at,
            created_at: l.created_at,
        }
    }
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
    async fn save_segment(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        definition: Json<Value>,
        #[graphql(default = true)] enabled: bool,
        #[graphql(default = false)] fire_existing: bool,
    ) -> async_graphql::Result<Segment> {
        admin(ctx, project_id)?;
        Ok(store(ctx)?
            .save_segment(project_id, definition.0, enabled, fire_existing)
            .await
            .map_err(error)?
            .into())
    }
    async fn delete_segment(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .delete_segment(project_id, id)
            .await
            .map_err(error)
    }
    async fn save_webhook(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        input: WebhookInput,
    ) -> async_graphql::Result<Webhook> {
        admin(ctx, project_id)?;
        let WebhookInput {
            segment_id,
            name,
            kind,
            config,
            enabled,
        } = input;
        let parsed =
            clickman_destinations::WebhookConfig::parse(project_id, config.0).map_err(|_| {
                async_graphql::Error::new("invalid webhook configuration")
                    .extend_with(|_, e| e.set("code", "BAD_USER_INPUT"))
            })?;
        let config = serde_json::to_value(parsed).expect("webhook configuration");
        if let Some(policy) = ctx.data_opt::<std::sync::Arc<dyn IntegrationPolicy>>() {
            policy
                .validate(project_id, &kind, &config)
                .await
                .map_err(error)?;
        }
        Ok(store(ctx)?
            .save_webhook(project_id, segment_id, &name, &kind, config, enabled)
            .await
            .map_err(error)?
            .into())
    }
    async fn delete_webhook(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .delete_webhook(project_id, id)
            .await
            .map_err(error)
    }
    async fn replay_webhook(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        id: Uuid,
    ) -> async_graphql::Result<bool> {
        admin(ctx, project_id)?;
        store(ctx)?
            .replay_webhook(project_id, id)
            .await
            .map_err(error)
    }
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        slug: String,
        name: String,
    ) -> async_graphql::Result<Project> {
        let permissions = access(ctx)?;
        if let Some(creator) = ctx.data_opt::<std::sync::Arc<dyn ProjectCreator>>() {
            let mut project = Project::from(
                creator
                    .create(store(ctx)?, &slug, &name)
                    .await
                    .map_err(error)?,
            );
            // Scope the trusted capability's role to this returned object only.
            // Other roots and future requests keep their original Access.
            project.created_role = Some(creator.created_role());
            return Ok(project);
        }
        if !permissions.is_root() {
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
        if let Some(maximum) = ctx
            .data_opt::<ProjectPolicy>()
            .and_then(|p| p.max_retention_days)
            && settings.retention_days.is_none_or(|days| days > maximum)
        {
            return Err(error(StorageError::Invalid(format!(
                "This plan allows up to {maximum} days of retention."
            ))));
        }
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
        if ctx
            .data_opt::<ProjectPolicy>()
            .is_some_and(|p| !p.allow_access_keys)
        {
            return Err(denied(
                "This host uses member sessions for dashboard access.",
            ));
        }
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
        storage
            .evaluate_segments(project_id, now)
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
        let config = serde_json::to_value(validated).unwrap();
        if let Some(policy) = ctx.data_opt::<std::sync::Arc<dyn IntegrationPolicy>>() {
            policy
                .validate(project_id, "mixpanel", &config)
                .await
                .map_err(error)?;
        }
        let d = store(ctx)?
            .save_destination(project_id, &name, "mixpanel", config)
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
