use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(Initial)]
    }
}

struct Initial;

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
