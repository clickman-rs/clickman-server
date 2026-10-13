pub mod adapters;
mod config;
mod connection;
mod delivery;
pub mod entities;
mod identity;
mod observations;
mod segments;
mod webhooks;
pub use observations::ObservationQuery;
pub use segments::SavedSegment;
pub use webhooks::{Webhook, WebhookDelivery, WebhookLog};
mod management;
mod reports;

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    DbErr, Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub use adapters::{AnalyticsAdapter, PostgresAdapter};
use connection::Connection;
use std::ops::Deref;

pub use config::{Access, ProjectSettings, RateLimit, Role};
pub use delivery::{DeliveryPage, Destination, StoredEvent};
pub use management::{AccessKey, IssuedKey, Project, WriteIdentity, WriteKey};
pub use reports::Report;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Codec(#[from] clickman_analytics::CodecError),
    #[error("{0}")]
    Invalid(String),
    #[error("ingestion capacity exhausted")]
    Capacity { retry_after_seconds: u64 },
    #[error("analytics {operation} failed: {source}")]
    Backend {
        operation: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct MetadataStore {
    db: Connection,
}

impl MetadataStore {
    pub fn transaction_scoped(&self) -> bool {
        matches!(self.db, Connection::Transaction(_))
    }
    pub async fn connect(url: &str, pool_size: u32) -> Result<Self> {
        if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
            return Err(Error::Invalid("Clickman requires PostgreSQL".into()));
        }
        if pool_size == 0 {
            return Err(Error::Invalid("database pool must be positive".into()));
        }
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(pool_size)
            .sqlx_logging(false)
            .connect_timeout(std::time::Duration::from_secs(10));
        Ok(Self {
            db: Connection::Pool(Database::connect(options).await?),
        })
    }
    pub fn from_connection(db: DatabaseConnection) -> Result<Self> {
        if db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::Invalid("Clickman requires PostgreSQL".into()));
        }
        Ok(Self {
            db: Connection::Pool(db),
        })
    }

    /// Use the caller's PostgreSQL transaction for every storage operation.
    /// Methods that open transactions use SeaORM savepoints; they do not commit
    /// the outer transaction. Do not run those methods concurrently on one view.
    /// Drop all cloned views before unwrapping the Arc to commit or roll back.
    pub fn from_transaction(tx: Arc<DatabaseTransaction>) -> Result<Self> {
        if tx.get_database_backend() != DbBackend::Postgres {
            return Err(Error::Invalid("Clickman requires PostgreSQL".into()));
        }
        Ok(Self {
            db: Connection::Transaction(tx),
        })
    }

    pub fn connection(
        &self,
    ) -> &(impl ConnectionTrait + TransactionTrait<Transaction = DatabaseTransaction>) {
        &self.db
    }
    pub async fn migrate(&self) -> Result<()> {
        let tx = self.db.begin().await?;
        tx.execute_unprepared("SELECT pg_advisory_xact_lock(1129073485)")
            .await?;
        clickman_migrations::Migrator::up(&tx, None).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn healthy(&self) -> bool {
        self.db.execute_unprepared("SELECT 1").await.is_ok()
    }
    pub async fn close(self) -> Result<()> {
        match self.db {
            Connection::Pool(db) => {
                db.close().await?;
                Ok(())
            }
            Connection::Transaction(_) => Err(Error::Invalid(
                "transaction-scoped storage cannot close the caller's transaction; commit or roll back the transaction owner".into(),
            )),
        }
    }
}

/// Public engine store: transactional metadata plus an interchangeable analytics backend.
#[derive(Clone)]
pub struct Storage {
    metadata: MetadataStore,
    adapter: Arc<dyn AnalyticsAdapter>,
}

impl Deref for Storage {
    type Target = MetadataStore;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl Storage {
    pub fn from_metadata(metadata: MetadataStore) -> Self {
        Self {
            metadata,
            adapter: Arc::new(PostgresAdapter),
        }
    }
    pub async fn connect(url: &str, pool_size: u32) -> Result<Self> {
        Ok(Self::from_metadata(
            MetadataStore::connect(url, pool_size).await?,
        ))
    }
    pub fn from_connection(db: DatabaseConnection) -> Result<Self> {
        Ok(Self::from_metadata(MetadataStore::from_connection(db)?))
    }
    pub fn from_transaction(tx: Arc<DatabaseTransaction>) -> Result<Self> {
        Ok(Self::from_metadata(MetadataStore::from_transaction(tx)?))
    }
    pub fn in_transaction(&self, tx: Arc<DatabaseTransaction>) -> Result<Self> {
        Ok(Self {
            metadata: MetadataStore::from_transaction(tx)?,
            adapter: self.adapter.clone(),
        })
    }
    pub fn with_analytics_adapter(mut self, adapter: Arc<dyn AnalyticsAdapter>) -> Self {
        self.adapter = adapter;
        self
    }
    pub fn backend_name(&self) -> &'static str {
        self.adapter.name()
    }
    pub async fn migrate(&self) -> Result<()> {
        self.metadata.migrate().await?;
        self.adapter.migrate(&self.metadata).await
    }
    pub async fn healthy(&self) -> bool {
        self.metadata.healthy().await && self.adapter.healthy(&self.metadata).await.is_ok()
    }
    pub async fn close(self) -> Result<()> {
        self.metadata.close().await
    }
    pub async fn insert(
        &self,
        identity: &WriteIdentity,
        events: &[clickman_protocol::Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        self.adapter
            .insert(&self.metadata, identity, events, received_at)
            .await
    }
    pub async fn insert_server(
        &self,
        project: Uuid,
        source: &str,
        events: &[clickman_protocol::Event],
        received_at: DateTime<Utc>,
    ) -> Result<u64> {
        self.adapter
            .insert_server(&self.metadata, project, source, events, received_at)
            .await
    }
    pub async fn rotate(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        self.adapter.rotate(&self.metadata, project, now).await
    }
    pub async fn prune(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        self.adapter.prune(&self.metadata, project, now).await
    }
    pub async fn erase(&self, project: Uuid, external: &str, now: DateTime<Utc>) -> Result<bool> {
        self.adapter
            .erase(&self.metadata, project, external, now)
            .await
    }
    pub async fn refresh_reports(&self, project: Uuid, now: DateTime<Utc>) -> Result<()> {
        self.adapter
            .refresh_reports(&self.metadata, project, now)
            .await
    }
}

pub fn sql(text: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, text, values)
}
pub fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub fn new_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

pub(crate) async fn lock_project(db: &impl ConnectionTrait, project: Uuid) -> Result<Project> {
    let row = db
        .query_one_raw(sql(
            "SELECT * FROM clickman_projects WHERE id=$1 FOR UPDATE",
            vec![project.into()],
        ))
        .await?
        .ok_or_else(|| Error::Invalid("project not found".into()))?;
    use sea_orm::FromQueryResult;
    Ok(Project::from_query_result(&row, "")?)
}

pub(crate) fn utc(value: time::OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(value.unix_timestamp(), value.nanosecond())
        .expect("protocol validated timestamp")
}
