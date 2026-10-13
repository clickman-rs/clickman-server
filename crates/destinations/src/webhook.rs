use crate::{EnvironmentSecrets, Error, Result, SecretResolver, validate_reference};
use clickman_storage::Webhook;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub endpoint_ref: Option<String>,
    pub signing_ref: String,
}
impl WebhookConfig {
    pub fn parse(project: Uuid, value: Value) -> Result<Self> {
        let config: Self = serde_json::from_value(value)
            .map_err(|_| Error("invalid webhook configuration".into()))?;
        if config.url.is_some() == config.endpoint_ref.is_some() {
            return Err(Error("choose either url or endpoint_ref".into()));
        }
        validate_reference(project, &config.signing_ref)?;
        if let Some(reference) = &config.endpoint_ref {
            validate_reference(project, reference)?;
        }
        if let Some(url) = &config.url {
            endpoint(url)?;
        }
        Ok(config)
    }
}
fn endpoint(value: &str) -> Result<Url> {
    if value.len() > 2048 {
        return Err(Error("webhook endpoint is too long".into()));
    }
    let url = Url::parse(value).map_err(|_| Error("invalid webhook endpoint".into()))?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error(
            "webhook endpoint must be HTTP(S) without embedded credentials or fragment".into(),
        ));
    }
    Ok(url)
}
fn public(address: IpAddr) -> bool {
    let address = match address {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
        _ => address,
    };
    if matches!(address, IpAddr::V6(_))
        && !"2000::/3"
            .parse::<ipnet::IpNet>()
            .expect("global IPv6 range")
            .contains(&address)
    {
        return false;
    }
    let blocked = &[
        "0.0.0.0/8",
        "10.0.0.0/8",
        "100.64.0.0/10",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "172.16.0.0/12",
        "192.0.0.0/24",
        "192.0.2.0/24",
        "192.88.99.0/24",
        "192.168.0.0/16",
        "198.18.0.0/15",
        "198.51.100.0/24",
        "203.0.113.0/24",
        "224.0.0.0/4",
        "240.0.0.0/4",
        "::/128",
        "::1/128",
        "fc00::/7",
        "fe80::/10",
        "ff00::/8",
        "2001:db8::/32",
        "2001::/23",
        "2002::/16",
        "64:ff9b:1::/48",
    ];
    !blocked.iter().any(|cidr| {
        cidr.parse::<ipnet::IpNet>()
            .expect("fixed address ranges")
            .contains(&address)
    })
}
pub fn signature(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    format!(
        "t={timestamp},v1={}",
        hex::encode(mac.finalize().into_bytes())
    )
}
#[derive(Clone)]
pub struct WebhookSender {
    secrets: Arc<dyn SecretResolver>,
    allow_private: bool,
}
impl Default for WebhookSender {
    fn default() -> Self {
        Self {
            secrets: Arc::new(EnvironmentSecrets),
            allow_private: false,
        }
    }
}
impl WebhookSender {
    pub fn with_secrets(mut self, secrets: Arc<dyn SecretResolver>) -> Self {
        self.secrets = secrets;
        self
    }
    /// Explicit operator opt-in for self-hosted internal endpoints/test fixtures.
    /// Cloud never enables this; project configuration cannot override it.
    pub fn allow_private_networks(mut self, allow: bool) -> Self {
        self.allow_private = allow;
        self
    }
    pub async fn deliver(&self, hook: &Webhook, payload: &Value) -> Result<u16> {
        let config = WebhookConfig::parse(hook.project_id, hook.config.clone())?;
        let value = match config.url {
            Some(url) => url,
            None => {
                self.secrets
                    .resolve(
                        hook.project_id,
                        config
                            .endpoint_ref
                            .as_deref()
                            .expect("validated endpoint reference"),
                    )
                    .await?
            }
        };
        let url = endpoint(&value)?;
        if !self.allow_private && url.scheme() != "https" {
            return Err(Error("public webhooks require HTTPS".into()));
        }
        let host = url
            .host_str()
            .expect("endpoint host")
            .trim_matches(['[', ']']);
        let port = url.port_or_known_default().expect("HTTP port");
        let addresses: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else {
            tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::lookup_host((host, port)),
            )
            .await
            .map_err(|_| Error("webhook DNS deadline".into()))?
            .map_err(|_| Error("webhook DNS failed".into()))?
            .collect()
        };
        if addresses.is_empty()
            || (!self.allow_private && addresses.iter().any(|a| !public(a.ip())))
        {
            return Err(Error(
                "webhook endpoint is not on an allowed public network".into(),
            ));
        }
        // Pin the validated addresses for this exchange, including DNS aliases;
        // a second resolution cannot redirect the request into a private subnet.
        let client = reqwest::Client::builder()
            .no_proxy()
            .resolve_to_addrs(host, &addresses)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| Error("webhook transport unavailable".into()))?;
        let secret = self
            .secrets
            .resolve(hook.project_id, &config.signing_ref)
            .await?;
        if secret.len() < 16 {
            return Err(Error(
                "webhook signing key must contain at least 16 bytes".into(),
            ));
        }
        let body = serde_json::to_vec(payload)
            .map_err(|_| Error("webhook payload serialization failed".into()))?;
        let event = payload["id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| Error("webhook event identity is invalid".into()))?;
        let timestamp = chrono::Utc::now().timestamp();
        let response = client
            .post(url)
            .header("content-type", "application/json")
            .header("x-clickman-event-id", event.to_string())
            .header("x-clickman-signature", signature(&secret, timestamp, &body))
            .body(body)
            .send()
            .await
            .map_err(|_| Error("webhook exchange failed".into()))?;
        let status = response.status().as_u16();
        Ok(status)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation_and_network_policy_block_private_and_metadata_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "192.0.2.1",
        ] {
            assert!(!public(ip.parse().unwrap()));
        }
        assert!(public("8.8.8.8".parse().unwrap()));
        for url in [
            "file:///etc/passwd",
            "http://user:secret@host.test/",
            "https://host.test/#fragment",
        ] {
            assert!(endpoint(url).is_err());
        }
        assert_eq!(
            signature("0123456789abcdef", 10, b"body"),
            "t=10,v1=d9afd5853cd4023fc11cab99490599633856537a4a64c6d661af598ffeb6a0ad"
        );
        assert_ne!(
            signature("0123456789abcdef", 10, b"body"),
            signature("0123456789abcdef", 11, b"body")
        );
    }
}
