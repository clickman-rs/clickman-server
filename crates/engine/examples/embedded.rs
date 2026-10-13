use axum::{Router, extract::State, routing::get};
use clickman_engine::{Engine, StandaloneAuthenticator};
use clickman_storage::Storage;
use std::sync::Arc;

#[derive(Clone)]
struct Host {
    name: Arc<str>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::connect(&std::env::var("DATABASE_URL")?, 10).await?;
    storage.migrate().await?;
    // Replace this authenticator with the host's cookies/JWT/session verifier.
    let auth =
        StandaloneAuthenticator::new(storage.clone(), &std::env::var("CLICKMAN_ADMIN_TOKEN")?)?;
    let engine = Engine::new(storage)?.with_authenticator(Arc::new(auth));
    let app = Router::new()
        .route(
            "/",
            get(|State(host): State<Host>| async move { format!("{} with Clickman", host.name) }),
        )
        .with_state(Host {
            name: "Axum host".into(),
        })
        .nest("/analytics/", engine.router_with_dashboard("apps/web/dist"));
    let jobs = engine.jobs().clone();
    let background = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(error) = jobs.tick().await {
                tracing::error!(%error,"maintenance failed");
            }
        }
    });
    let bind = std::env::var("CLICKMAN_BIND").unwrap_or_else(|_| "127.0.0.1:4131".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(clickman_engine::shutdown_signal())
    .await?;
    background.abort();
    Ok(())
}
