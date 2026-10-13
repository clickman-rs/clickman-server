use super::*;
use crate::{Error, Storage, connection::Connection};
use sea_orm::{DbBackend, MockDatabase};
use std::sync::{Arc, Mutex};

struct Probe {
    calls: Mutex<Vec<&'static str>>,
}
#[async_trait]
impl AnalyticsAdapter for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }
    async fn insert(
        &self,
        _: &MetadataStore,
        _: &WriteIdentity,
        _: &[Event],
        _: DateTime<Utc>,
    ) -> Result<u64> {
        self.calls.lock().unwrap().push("insert");
        Ok(3)
    }
    async fn insert_server(
        &self,
        _: &MetadataStore,
        _: Uuid,
        _: &str,
        _: &[Event],
        _: DateTime<Utc>,
    ) -> Result<u64> {
        self.calls.lock().unwrap().push("trusted");
        Ok(4)
    }
    async fn rotate(&self, _: &MetadataStore, _: Uuid, _: DateTime<Utc>) -> Result<u64> {
        self.calls.lock().unwrap().push("rotate");
        Ok(3)
    }
    async fn prune(&self, _: &MetadataStore, _: Uuid, _: DateTime<Utc>) -> Result<u64> {
        self.calls.lock().unwrap().push("prune");
        Ok(2)
    }
    async fn erase(&self, _: &MetadataStore, _: Uuid, _: &str, _: DateTime<Utc>) -> Result<bool> {
        self.calls.lock().unwrap().push("erase");
        Ok(true)
    }
    async fn refresh_reports(&self, _: &MetadataStore, _: Uuid, _: DateTime<Utc>) -> Result<()> {
        self.calls.lock().unwrap().push("reports");
        Err(Error::Invalid("backend unavailable".into()))
    }
}

#[tokio::test]
async fn supplied_backend_owns_analytics_and_its_failure_is_propagated() {
    // No database: any accidental PostgreSQL execution fails this contract test.
    let metadata = MetadataStore {
        db: Connection::Pool(MockDatabase::new(DbBackend::Postgres).into_connection()),
    };
    let storage = Storage::from_metadata(metadata);
    assert_eq!(storage.backend_name(), "postgres");
    let probe = Arc::new(Probe {
        calls: Mutex::new(vec![]),
    });
    let storage = storage.with_analytics_adapter(probe.clone());
    let project = Uuid::new_v4();
    let now = Utc::now();
    let identity = WriteIdentity {
        key_id: Uuid::new_v4(),
        project_id: project,
        source: "web".into(),
        settings: Default::default(),
    };
    assert_eq!(storage.backend_name(), "probe");
    assert_eq!(storage.insert(&identity, &[], now).await.unwrap(), 3);
    assert_eq!(
        storage
            .insert_server(project, "server", &[], now)
            .await
            .unwrap(),
        4
    );
    assert_eq!(storage.rotate(project, now).await.unwrap(), 3);
    assert_eq!(storage.prune(project, now).await.unwrap(), 2);
    assert!(storage.erase(project, "actor", now).await.unwrap());
    let error = storage.refresh_reports(project, now).await.unwrap_err();
    assert_eq!(error.to_string(), "backend unavailable");
    assert_eq!(
        *probe.calls.lock().unwrap(),
        ["insert", "trusted", "rotate", "prune", "erase", "reports"]
    );
}
