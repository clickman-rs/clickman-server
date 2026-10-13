use anyhow::{Context, Result};
use apalis::prelude::*;
use apalis_cron::{CronStream, Schedule};
use clickman_engine::Engine;
use clickman_jobs::Jobs;
use std::str::FromStr;

#[derive(Default, Debug, Clone)]
struct Maintenance;

async fn tick(_: Maintenance, jobs: Data<Jobs>) {
    if let Err(error) = jobs.tick().await {
        tracing::error!(%error,"maintenance tick failed");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clickman=info,apalis=info".into()),
        )
        .init();
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let storage = clickman_storage::Storage::connect(&url, 4)
        .await?
        .with_analytics_adapter(clickman_storage::adapters::from_environment()?);
    storage.migrate().await?;
    let engine = Engine::new(storage)?;
    // Run immediately after restart; missed cron ticks are not durable work.
    engine.jobs().tick().await?;
    let schedule = Schedule::from_str(
        &std::env::var("CLICKMAN_MAINTENANCE_CRON").unwrap_or_else(|_| "0 * * * * *".into()),
    )
    .context("invalid CLICKMAN_MAINTENANCE_CRON")?;
    let worker = WorkerBuilder::new("clickman-maintenance")
        .data(engine.jobs().clone())
        .backend(CronStream::new(schedule))
        .build_fn(tick);
    tokio::select! { _=worker.run()=>{}, _=clickman_engine::shutdown_signal()=>{} }
    Ok(())
}
