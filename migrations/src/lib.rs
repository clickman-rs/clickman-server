use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DbBackend, Statement};

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(Initial),
            Box::new(ProjectNamespaces),
            Box::new(Identities),
            Box::new(Segments),
            Box::new(ActorActivity),
        ]
    }
}

struct Initial;
struct ActorActivity;
impl MigrationName for ActorActivity {
    fn name(&self) -> &str {
        "m20261013_000005_actor_activity"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for ActorActivity {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../005_actor_activity.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("LOCK TABLE clickman_actors IN ACCESS EXCLUSIVE MODE")
            .await?;
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT EXISTS(SELECT 1 FROM clickman_actors WHERE NOT has_events) AS placeholders",
            ))
            .await?
            .expect("actor activity count");
        if row.try_get::<bool>("", "placeholders")? {
            return Err(DbErr::Custom(
                "identity-only actors must be erased before rolling back activity markers".into(),
            ));
        }
        db.execute_unprepared("ALTER TABLE clickman_actors DROP COLUMN has_events")
            .await?;
        Ok(())
    }
}
struct Segments;
impl MigrationName for Segments {
    fn name(&self) -> &str {
        "m20261013_000004_segments"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for Segments {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../004_segments.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("LOCK TABLE clickman_segments IN ACCESS EXCLUSIVE MODE")
            .await?;
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT EXISTS(SELECT 1 FROM clickman_segments) AS populated",
            ))
            .await?
            .expect("segment count");
        if row.try_get::<bool>("", "populated")? {
            return Err(DbErr::Custom(
                "segments and delivery history must be erased before rollback".into(),
            ));
        }
        db.execute_unprepared("DROP TABLE clickman_webhook_deliveries, clickman_webhooks, clickman_segment_entries, clickman_segment_members, clickman_segments")
            .await?;
        Ok(())
    }
}
struct Identities;
impl MigrationName for Identities {
    fn name(&self) -> &str {
        "m20261013_000003_identities"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for Identities {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../003_identities.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("LOCK TABLE clickman_actors IN ACCESS EXCLUSIVE MODE")
            .await?;
        let row=manager.get_connection().query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT EXISTS(SELECT 1 FROM clickman_actors WHERE merged_into_id IS NOT NULL) AS linked")).await?.expect("identity count");
        if row.try_get::<bool>("", "linked")? {
            return Err(DbErr::Custom(
                "linked identities must be erased before rolling back identity support".into(),
            ));
        }
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE clickman_actors DROP COLUMN merged_into_id")
            .await?;
        Ok(())
    }
}

struct ProjectNamespaces;
impl MigrationName for ProjectNamespaces {
    fn name(&self) -> &str {
        "m20261013_000002_project_namespaces"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for ProjectNamespaces {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../002_project_namespaces.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("LOCK TABLE clickman_projects IN ACCESS EXCLUSIVE MODE")
            .await?;
        let populated = manager
            .get_connection()
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT EXISTS(SELECT 1 FROM clickman_projects WHERE namespace<>'') AS populated",
            ))
            .await?
            .expect("namespace count");
        if populated.try_get::<bool>("", "populated")? {
            return Err(DbErr::Custom(
                "remove namespaced projects before rolling back project namespaces".into(),
            ));
        }
        manager.get_connection().execute_unprepared("ALTER TABLE clickman_projects DROP CONSTRAINT clickman_projects_namespace_slug_key; ALTER TABLE clickman_projects DROP COLUMN namespace; ALTER TABLE clickman_projects ADD CONSTRAINT clickman_projects_slug_key UNIQUE(slug)").await?;
        Ok(())
    }
}

impl MigrationName for Initial {
    fn name(&self) -> &str {
        "m20261009_000001_initial"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Initial {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../001_initial.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "deliveries",
            "destinations",
            "reports",
            "funnels",
            "daily_counts",
            "actor_days",
            "chunks",
            "actors",
            "keys",
            "event_names",
            "events",
            "access_keys",
            "write_keys",
            "projects",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE clickman_{table}"))
                .await?;
        }
        Ok(())
    }
}
