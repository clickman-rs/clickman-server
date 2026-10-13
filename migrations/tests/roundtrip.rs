use clickman_migrations::Migrator;
use sea_orm_migration::{
    MigratorTrait,
    sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement},
};
use uuid::Uuid;

async fn owned_database() -> DatabaseConnection {
    assert_eq!(std::env::var("CLICKMAN_TEST_OWNED").as_deref(), Ok("1"));
    let base = std::env::var("TEST_DATABASE_URL").expect("use scripts/test-platform");
    let name = format!("clickman_migration_{}", Uuid::new_v4().simple());
    let owner = Database::connect(&base).await.unwrap();
    owner
        .execute_unprepared(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    let (server, _) = base.rsplit_once('/').unwrap();
    owner.close().await.unwrap();
    // The runner destroys the entire owned cluster, including this isolated database.
    Database::connect(format!("{server}/{name}")).await.unwrap()
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn empty_schema_can_migrate_down_and_up_repeatedly() {
    let db = owned_database().await;
    for _ in 0..2 {
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            Migrator::get_applied_migrations(&db).await.unwrap().len(),
            5
        );
        Migrator::down(&db, None).await.unwrap();
        assert!(
            Migrator::get_applied_migrations(&db)
                .await
                .unwrap()
                .is_empty()
        );
    }
    Migrator::up(&db, None).await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn rollback_refuses_to_lose_identity_or_segment_data() {
    let db = owned_database().await;
    Migrator::up(&db, None).await.unwrap();
    let project = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO clickman_projects(id,slug,name,settings) VALUES($1,'rollback','Rollback','{}')",
        [project.into()],
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO clickman_actors(project_id,external_id,first_seen_on,has_events) VALUES($1,'identity-only',CURRENT_DATE,false)",
        [project.into()],
    ))
    .await
    .unwrap();
    assert!(Migrator::down(&db, Some(1)).await.is_err());
    assert_eq!(
        Migrator::get_applied_migrations(&db).await.unwrap().len(),
        5
    );
    db.execute_unprepared("DELETE FROM clickman_actors")
        .await
        .unwrap();
    Migrator::down(&db, Some(1)).await.unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO clickman_segments(id,project_id,key,definition,enabled) VALUES($1,$2,'buyers','{}',false)",
        [Uuid::new_v4().into(), project.into()],
    ))
    .await
    .unwrap();
    assert!(Migrator::down(&db, Some(1)).await.is_err());
    db.execute_unprepared("DELETE FROM clickman_segments")
        .await
        .unwrap();
    Migrator::down(&db, Some(1)).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db.close().await.unwrap();
}
