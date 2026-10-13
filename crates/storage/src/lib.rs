mod config;
mod connection;
mod delivery;
pub mod entities;
mod management;
mod reports;
mod rotation;

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    DbErr, Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use connection::Connection;

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
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct Storage {
    db: Connection,
}

impl Storage {
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
