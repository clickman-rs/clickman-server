use rmcp::model::CallToolResult;
use serde_json::json;

/// Deliberately contains no HTTP body, URL, credentials or remote error text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Configuration(&'static str),
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("The host rejected authentication. Renew the configured credential or session.")]
    Unauthorized,
    #[error("The current host identity cannot access this project or operation.")]
    Forbidden,
    #[error("The host rejected the definition. Check it in the dashboard before retrying.")]
    Rejected,
    #[error("The host has reached its request or usage limit. Retry later.")]
    RateLimited,
    #[error("The GraphQL host did not return a valid bounded response.")]
    InvalidResponse,
    #[error("The host response exceeded the 2 MiB limit. Request fewer report keys.")]
    ResponseTooLarge,
    #[error("The GraphQL host is unavailable or did not respond within 30 seconds.")]
    Unavailable,
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "CONFIGURATION",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::Rejected => "REJECTED",
            Self::RateLimited => "RATE_LIMITED",
            Self::InvalidResponse => "INVALID_RESPONSE",
            Self::ResponseTooLarge => "RESPONSE_TOO_LARGE",
            Self::Unavailable => "UNAVAILABLE",
        }
    }

    pub(crate) fn tool_result(self) -> CallToolResult {
        CallToolResult::structured_error(json!({"error":self.code(),"message":self.to_string()}))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
