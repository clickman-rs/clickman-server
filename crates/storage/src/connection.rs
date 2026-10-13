use async_trait::async_trait;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    ExecResult, IsolationLevel, QueryResult, Statement, TransactionError, TransactionOptions,
    TransactionTrait,
};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub(crate) enum Connection {
    Pool(DatabaseConnection),
    Transaction(Arc<DatabaseTransaction>),
}

#[async_trait]
impl ConnectionTrait for Connection {
    fn get_database_backend(&self) -> DbBackend {
        match self {
            Self::Pool(db) => db.get_database_backend(),
            Self::Transaction(tx) => tx.get_database_backend(),
        }
    }

    async fn execute_raw(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match self {
            Self::Pool(db) => db.execute_raw(statement).await,
            Self::Transaction(tx) => tx.execute_raw(statement).await,
        }
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        match self {
            Self::Pool(db) => db.execute_unprepared(sql).await,
            Self::Transaction(tx) => tx.execute_unprepared(sql).await,
        }
    }

    async fn query_one_raw(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        match self {
            Self::Pool(db) => db.query_one_raw(statement).await,
            Self::Transaction(tx) => tx.query_one_raw(statement).await,
        }
    }

    async fn query_all_raw(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match self {
            Self::Pool(db) => db.query_all_raw(statement).await,
            Self::Transaction(tx) => tx.query_all_raw(statement).await,
        }
    }
}

#[async_trait]
impl TransactionTrait for Connection {
    type Transaction = DatabaseTransaction;

    async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        match self {
            Self::Pool(db) => db.begin().await,
            Self::Transaction(tx) => tx.begin().await,
        }
    }

    async fn begin_with_config(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<DatabaseTransaction, DbErr> {
        match self {
            Self::Pool(db) => db.begin_with_config(isolation_level, access_mode).await,
            Self::Transaction(tx) => tx.begin_with_config(isolation_level, access_mode).await,
        }
    }

    async fn begin_with_options(
        &self,
        options: TransactionOptions,
    ) -> Result<DatabaseTransaction, DbErr> {
        match self {
            Self::Pool(db) => db.begin_with_options(options).await,
            Self::Transaction(tx) => tx.begin_with_options(options).await,
        }
    }

    async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        match self {
            Self::Pool(db) => db.transaction(callback).await,
            Self::Transaction(tx) => tx.transaction(callback).await,
        }
    }

    async fn transaction_with_config<F, T, E>(
        &self,
        callback: F,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        match self {
            Self::Pool(db) => {
                db.transaction_with_config(callback, isolation_level, access_mode)
                    .await
            }
            Self::Transaction(tx) => {
                tx.transaction_with_config(callback, isolation_level, access_mode)
                    .await
            }
        }
    }
}
