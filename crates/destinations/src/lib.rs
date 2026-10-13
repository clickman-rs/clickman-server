use async_trait::async_trait;
use clickman_storage::{Destination, StoredEvent};
use flate2::{Compression, write::GzEncoder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
mod webhook;
pub use webhook::{WebhookConfig, WebhookSender, signature as webhook_signature};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;

/// Hosts may implement other destinations without changing the engine or worker.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    async fn deliver(&self, destination: &Destination, events: &[StoredEvent]) -> Result<()>;
}

/// Only project-scoped references are stored in integration configuration.
/// Embedded hosts may resolve them from a vault; standalone installations use
/// environment variables. Callers never receive another project's secrets.
#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, project: Uuid, reference: &str) -> Result<String>;
}
pub struct EnvironmentSecrets;
#[async_trait]
impl SecretResolver for EnvironmentSecrets {
    async fn resolve(&self, project: Uuid, reference: &str) -> Result<String> {
        validate_reference(project, reference)?;
        std::env::var(reference)
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error("integration credential is unavailable".into()))
    }
}
pub fn validate_reference(project: Uuid, reference: &str) -> Result<()> {
    let prefix = credential_prefix(project);
    if reference.len() > 100
        || !reference.starts_with(&prefix)
        || reference.len() == prefix.len()
        || !reference
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(Error(format!(
            "credential reference must start with {prefix}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    #[default]
    Us,
    Eu,
    In,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixpanelConfig {
    pub project_id: String,
    pub username_env: String,
    pub secret_env: String,
    #[serde(default)]
    pub region: Region,
}
impl MixpanelConfig {
    pub fn parse(project: Uuid, config: Value) -> Result<Self> {
        let config: Self = serde_json::from_value(config).map_err(|e| Error(e.to_string()))?;
        if config.project_id.is_empty()
            || config.project_id.len() > 100
            || !config.project_id.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error("Mixpanel project_id must be numeric".into()));
        }
        for key in [&config.username_env, &config.secret_env] {
            validate_reference(project, key)?;
        }
        Ok(config)
    }
}

/// A project administrator must not reference another project's or the host's secrets.
pub fn credential_prefix(project: Uuid) -> String {
    format!("CLICKMAN_{}_", project.simple().to_string().to_uppercase())
}

#[derive(Clone)]
pub struct Mixpanel {
    client: reqwest::Client,
    secrets: Arc<dyn SecretResolver>,
}
impl Mixpanel {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| Error(e.to_string()))?,
            secrets: Arc::new(EnvironmentSecrets),
        })
    }
    pub fn with_secrets(mut self, secrets: Arc<dyn SecretResolver>) -> Self {
        self.secrets = secrets;
        self
    }
}

pub fn mixpanel_record(event: &StoredEvent) -> Value {
    let mut properties = event.properties.as_object().cloned().unwrap_or_default();
    if let Some(context) = event.context.as_object() {
        properties.extend(
            context
                .iter()
                .map(|(k, v)| (format!("context.{k}"), v.clone())),
        );
    }
    properties.insert("time".into(), json!(event.occurred_at.timestamp_millis()));
    properties.insert(
        "distinct_id".into(),
        json!(if event.external_id == "*" {
            ""
        } else {
            &event.external_id
        }),
    );
    properties.insert("$insert_id".into(), json!(event.message_id));
    json!({"event":event.event,"properties":properties})
}

#[async_trait]
impl Dispatcher for Mixpanel {
    async fn deliver(&self, destination: &Destination, events: &[StoredEvent]) -> Result<()> {
        if destination.kind != "mixpanel" {
            return Err(Error("no dispatcher for destination kind".into()));
        }
        let config = MixpanelConfig::parse(destination.project_id, destination.config.clone())?;
        let username = self
            .secrets
            .resolve(destination.project_id, &config.username_env)
            .await?;
        let secret = self
            .secrets
            .resolve(destination.project_id, &config.secret_env)
            .await?;
        if username.is_empty() || secret.is_empty() {
            return Err(Error("Mixpanel credentials are empty".into()));
        }
        let host = match config.region {
            Region::Us => "api.mixpanel.com",
            Region::Eu => "api-eu.mixpanel.com",
            Region::In => "api-in.mixpanel.com",
        };
        let url = format!(
            "https://{host}/import?strict=1&project_id={}",
            config.project_id
        );
        let mut pages: Vec<Vec<Value>> = vec![vec![]];
        let mut bytes = 2usize;
        for event in events {
            let record = mixpanel_record(event);
            let size = serde_json::to_vec(&record)
                .map_err(|e| Error(e.to_string()))?
                .len();
            if size + 2 > 10 * 1024 * 1024 {
                return Err(Error("Mixpanel record exceeds import body limit".into()));
            }
            if bytes + size + 1 > 10 * 1024 * 1024 {
                pages.push(vec![]);
                bytes = 2;
            }
            pages.last_mut().unwrap().push(record);
            bytes += size + 1;
        }
        for page in pages.into_iter().filter(|p| !p.is_empty()) {
            let body = serde_json::to_vec(&page).map_err(|e| Error(e.to_string()))?;
            let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
            gzip.write_all(&body).map_err(|e| Error(e.to_string()))?;
            let response = self
                .client
                .post(&url)
                .basic_auth(&username, Some(&secret))
                .header("content-type", "application/json")
                .header("content-encoding", "gzip")
                .header("accept", "application/json")
                .body(gzip.finish().map_err(|e| Error(e.to_string()))?)
                .send()
                .await
                .map_err(|_| Error("Mixpanel exchange failed".into()))?;
            match response.status().as_u16() {
                200 => {}
                400 => {
                    tracing::warn!(project=%destination.project_id,destination=%destination.id,"Mixpanel permanently rejected part of an import page")
                }
                status => return Err(Error(format!("Mixpanel answered HTTP {status}"))),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_references_cannot_cross_project_or_host_boundaries() {
        let project = Uuid::new_v4();
        let config = json!({
            "project_id": "1234",
            "username_env": format!("{}MIXPANEL_USERNAME", credential_prefix(project)),
            "secret_env": format!("{}MIXPANEL_SECRET", credential_prefix(project)),
        });
        assert!(MixpanelConfig::parse(project, config.clone()).is_ok());
        assert!(MixpanelConfig::parse(Uuid::new_v4(), config.clone()).is_err());
        for host_secret in ["CLICKMAN_ADMIN_TOKEN", "DATABASE_URL"] {
            let mut malicious = config.clone();
            malicious["secret_env"] = json!(host_secret);
            assert!(MixpanelConfig::parse(project, malicious).is_err());
        }
    }
}
