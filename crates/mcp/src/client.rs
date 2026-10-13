use crate::{Error, Result};
use reqwest::header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue, ORIGIN};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use url::{Host, Url};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Operator-owned endpoint and credentials. Neither configuration nor client
/// implements Debug; authentication headers are marked sensitive.
pub struct ClientConfig {
    endpoint: Url,
    headers: HeaderMap,
    secrets: Vec<String>,
}

impl ClientConfig {
    pub fn new(endpoint: &str) -> Result<Self> {
        let endpoint = Url::parse(endpoint)
            .map_err(|_| Error::Configuration("Invalid GraphQL endpoint URL."))?;
        let loopback = match endpoint.host() {
            Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        };
        if !(endpoint.scheme() == "https" || endpoint.scheme() == "http" && loopback)
            || endpoint.host().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(Error::Configuration(
                "GraphQL endpoint must use HTTPS (HTTP only on loopback), without credentials, query or fragment.",
            ));
        }
        Ok(Self {
            endpoint,
            headers: HeaderMap::new(),
            secrets: Vec::new(),
        })
    }

    /// Standalone instance token or project access token; never a source key.
    pub fn with_token(mut self, token: &str) -> Result<Self> {
        if self.headers.contains_key(COOKIE) {
            return Err(Error::Configuration(
                "Choose token or session authentication, not both.",
            ));
        }
        if token.is_empty() || token.len() > 4096 || token.trim() != token {
            return Err(Error::Configuration("Invalid MCP token."));
        }
        let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| Error::Configuration("Invalid MCP token."))?;
        value.set_sensitive(true);
        self.headers.insert(AUTHORIZATION, value);
        self.secrets.push(token.to_owned());
        Ok(self)
    }

    /// Session-only hosts such as Clickman Cloud require their complete Cookie
    /// header and exact configured public Origin, including for read-only POSTs.
    pub fn with_session(mut self, cookie: &str, origin: &str) -> Result<Self> {
        if self.headers.contains_key(AUTHORIZATION) {
            return Err(Error::Configuration(
                "Choose token or session authentication, not both.",
            ));
        }
        if cookie.is_empty() || cookie.len() > 8192 || cookie.trim() != cookie {
            return Err(Error::Configuration("Invalid MCP session cookie."));
        }
        let parsed =
            Url::parse(origin).map_err(|_| Error::Configuration("Invalid MCP host Origin."))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.origin().ascii_serialization() != origin
        {
            return Err(Error::Configuration(
                "MCP host Origin must be an exact HTTP(S) origin.",
            ));
        }
        let mut value = HeaderValue::from_str(cookie)
            .map_err(|_| Error::Configuration("Invalid MCP session cookie."))?;
        value.set_sensitive(true);
        self.headers.insert(COOKIE, value);
        self.headers.insert(
            ORIGIN,
            HeaderValue::from_str(origin)
                .map_err(|_| Error::Configuration("Invalid MCP host Origin."))?,
        );
        self.secrets.push(cookie.to_owned());
        // Redact individual session values if an upstream ever echoes one.
        for pair in cookie.split(';') {
            if let Some((_, value)) = pair.trim().split_once('=')
                && !value.is_empty()
            {
                self.secrets.push(value.to_owned());
            }
        }
        Ok(self)
    }
}

#[derive(Clone)]
pub struct HttpGraphqlClient {
    endpoint: Url,
    http: reqwest::Client,
    secrets: Arc<Vec<String>>,
    permits: Arc<Semaphore>,
}

impl HttpGraphqlClient {
    pub fn new(config: ClientConfig) -> Result<Self> {
        if !config.headers.contains_key(AUTHORIZATION) && !config.headers.contains_key(COOKIE) {
            return Err(Error::Configuration(
                "MCP requires a token or an authenticated host session.",
            ));
        }
        let http = reqwest::Client::builder()
            .default_headers(config.headers)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("clickman-mcp/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::Configuration("Could not initialize the GraphQL HTTP client."))?;
        Ok(Self {
            endpoint: config.endpoint,
            http,
            secrets: Arc::new(config.secrets),
            permits: Arc::new(Semaphore::new(8)),
        })
    }

    /// Only crate-owned typed operations call this method. Model input never
    /// supplies a URL, headers or GraphQL document.
    pub(crate) async fn execute(&self, query: &'static str, variables: Value) -> Result<Value> {
        let body = serde_json::to_vec(&json!({"query":query,"variables":variables}))
            .map_err(|_| Error::InvalidInput("Could not encode the tool input."))?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(Error::InvalidInput("Tool input exceeds the 64 KiB limit."));
        }
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            let _permit = self
                .permits
                .acquire()
                .await
                .map_err(|_| Error::Unavailable)?;
            let mut response = self
                .http
                .post(self.endpoint.clone())
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
                .map_err(|_| Error::Unavailable)?;
            match response.status().as_u16() {
                401 => return Err(Error::Unauthorized),
                403 => return Err(Error::Forbidden),
                429 => return Err(Error::RateLimited),
                200..=299 => {}
                _ => return Err(Error::Unavailable),
            }
            if response
                .content_length()
                .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
            {
                return Err(Error::ResponseTooLarge);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
                if chunk.len() > MAX_RESPONSE_BYTES - bytes.len() {
                    return Err(Error::ResponseTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            let response: Value =
                serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)?;
            if let Some(errors) = response.get("errors").and_then(Value::as_array)
                && !errors.is_empty()
            {
                // Do not expose remote messages, input snippets or extensions:
                // hosts may include private diagnostics in GraphQL errors.
                let codes: Vec<_> = errors
                    .iter()
                    .filter_map(|error| error.pointer("/extensions/code").and_then(Value::as_str))
                    .collect();
                return Err(if codes.contains(&"FORBIDDEN") {
                    Error::Forbidden
                } else if codes.contains(&"UNAUTHORIZED") {
                    Error::Unauthorized
                } else if codes.contains(&"BAD_USER_INPUT") {
                    Error::Rejected
                } else if codes.contains(&"CAPACITY_EXHAUSTED") {
                    Error::RateLimited
                } else {
                    Error::InvalidResponse
                });
            }
            response
                .get("data")
                .filter(|data| data.is_object())
                .cloned()
                .ok_or(Error::InvalidResponse)
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }

    pub(crate) fn redact(&self, value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    let key = key.to_ascii_lowercase();
                    if [
                        "password",
                        "secret",
                        "token",
                        "authorization",
                        "cookie",
                        "api_key",
                        "private_key",
                        "signing_ref",
                        "endpoint_ref",
                        "username_ref",
                    ]
                    .iter()
                    .any(|part| key.contains(part))
                    {
                        *value = Value::String("[REDACTED]".into());
                    } else {
                        self.redact(value);
                    }
                }
            }
            Value::Array(values) => values.iter_mut().for_each(|value| self.redact(value)),
            Value::String(text) => {
                for secret in self.secrets.iter() {
                    if text.contains(secret) {
                        *text = text.replace(secret, "[REDACTED]");
                    }
                }
            }
            _ => {}
        }
    }
}
