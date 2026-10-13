use async_trait::async_trait;
use axum::http::{HeaderMap, header};
use clickman_storage::{Access, Result, Storage, digest};
use subtle::ConstantTimeEq;

#[async_trait]
pub trait Authenticator: Send + Sync {
    async fn authenticate(&self, headers: &HeaderMap) -> Result<Option<Access>>;
}

pub(crate) struct Deny;
#[async_trait]
impl Authenticator for Deny {
    async fn authenticate(&self, _: &HeaderMap) -> Result<Option<Access>> {
        Ok(None)
    }
}

pub struct StandaloneAuthenticator {
    storage: Storage,
    root_digest: String,
}
impl StandaloneAuthenticator {
    pub fn new(storage: Storage, root_token: &str) -> Result<Self> {
        if root_token.trim() != root_token || root_token.len() < 32 || root_token.len() > 4096 {
            return Err(clickman_storage::Error::Invalid(
                "CLICKMAN_ADMIN_TOKEN must contain 32..4096 bytes without surrounding whitespace"
                    .into(),
            ));
        }
        Ok(Self {
            storage,
            root_digest: digest(root_token),
        })
    }
}
#[async_trait]
impl Authenticator for StandaloneAuthenticator {
    async fn authenticate(&self, headers: &HeaderMap) -> Result<Option<Access>> {
        let Some(token) = bearer(headers) else {
            return Ok(None);
        };
        if bool::from(digest(token).as_bytes().ct_eq(self.root_digest.as_bytes())) {
            return Ok(Some(Access::root()));
        }
        self.storage.access_identity(token).await
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() && token.len() <= 4096)
        .then_some(token.trim())
}
