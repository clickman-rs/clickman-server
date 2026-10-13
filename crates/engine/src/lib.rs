mod auth;
pub mod body;
mod http;
mod limiter;
mod track;

use axum::Router;
use clickman_destinations::{Dispatcher, Mixpanel};
use clickman_graphql::ClickmanSchema;
use clickman_jobs::Jobs;
use clickman_storage::{Result, Storage};
use std::sync::Arc;

pub use auth::{Authenticator, StandaloneAuthenticator};
pub use clickman_storage::{Access, Role};
pub use http::HTTP_ROUTES;
pub use track::TrackingEvent;

#[derive(Clone)]
pub struct Engine {
    storage: Storage,
    schema: ClickmanSchema,
    jobs: Jobs,
    authenticator: Arc<dyn Authenticator>,
    limiter: Arc<limiter::RateLimiter>,
    trust_proxy_headers: bool,
}

pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

impl Engine {
    pub fn new(storage: Storage) -> Result<Self> {
        let dispatcher =
            Arc::new(Mixpanel::new().map_err(|e| clickman_storage::Error::Invalid(e.to_string()))?);
        Ok(Self {
            schema: clickman_graphql::schema(storage.clone()),
            jobs: Jobs::new(storage.clone(), dispatcher),
            storage,
            authenticator: Arc::new(auth::Deny),
            limiter: Arc::new(limiter::RateLimiter::default()),
            trust_proxy_headers: false,
        })
    }
    pub fn with_authenticator(mut self, authenticator: Arc<dyn Authenticator>) -> Self {
        self.authenticator = authenticator;
        self
    }
    pub fn with_dispatcher(mut self, dispatcher: Arc<dyn Dispatcher>) -> Self {
        self.jobs = Jobs::new(self.storage.clone(), dispatcher);
        self
    }
    /// Enable only when a trusted reverse proxy overwrites X-Forwarded-For.
    pub fn trust_proxy_headers(mut self, trust: bool) -> Self {
        self.trust_proxy_headers = trust;
        self
    }
    pub fn storage(&self) -> &Storage {
        &self.storage
    }
    pub fn schema(&self) -> &ClickmanSchema {
        &self.schema
    }
    pub fn jobs(&self) -> &Jobs {
        &self.jobs
    }
    pub fn router(&self) -> Router {
        http::router(self.clone())
    }
    pub fn router_with_dashboard(&self, web_dir: impl AsRef<std::path::Path>) -> Router {
        let dir = web_dir.as_ref();
        self.router().fallback_service(
            tower_http::services::ServeDir::new(dir)
                .not_found_service(tower_http::services::ServeFile::new(dir.join("index.html"))),
        )
    }
}
