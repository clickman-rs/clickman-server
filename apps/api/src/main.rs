use anyhow::{Context, Result, bail};
use clickman_engine::{Engine, StandaloneAuthenticator};
use clickman_storage::Storage;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if command == "schema" {
        print!("{}", clickman_graphql::sdl());
        return Ok(());
    }
    if command != "serve" && command != "migrate" {
        bail!("usage: clickman-api [serve|migrate|schema]");
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clickman=info,tower_http=info".into()),
        )
        .init();
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = std::env::var("CLICKMAN_DATABASE_POOL")
        .unwrap_or_else(|_| "10".into())
        .parse()
        .context("invalid CLICKMAN_DATABASE_POOL")?;
    let storage = Storage::connect(&url, pool)
        .await?
        .with_analytics_adapter(clickman_storage::adapters::from_environment()?);
    storage.migrate().await?;
    if command == "migrate" {
        return Ok(());
    }
    let token =
        std::env::var("CLICKMAN_ADMIN_TOKEN").context("CLICKMAN_ADMIN_TOKEN is required")?;
    let authenticator = StandaloneAuthenticator::new(storage.clone(), &token)?;
    let trust_proxy = std::env::var("CLICKMAN_TRUST_PROXY_HEADERS").is_ok_and(|v| v == "1");
    let engine = Engine::new(storage)?
        .with_authenticator(Arc::new(authenticator))
        .trust_proxy_headers(trust_proxy);
    let web =
        PathBuf::from(std::env::var("CLICKMAN_WEB_DIR").unwrap_or_else(|_| "apps/web/dist".into()));
    let router = if web.join("index.html").is_file() {
        engine.router_with_dashboard(&web)
    } else {
        tracing::warn!("dashboard assets are absent; build apps/web or set CLICKMAN_WEB_DIR");
        engine.router()
    };
    let bind: SocketAddr = std::env::var("CLICKMAN_BIND")
        .unwrap_or_else(|_| "127.0.0.1:4132".into())
        .parse()
        .context("invalid CLICKMAN_BIND")?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind,"Clickman API listening");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(clickman_engine::shutdown_signal())
    .await?;
    Ok(())
}
