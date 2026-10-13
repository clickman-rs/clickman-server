use crate::{
    Engine,
    auth::bearer,
    body::{self, BodyError, MAX_WIRE_BYTES},
    limiter::Decision,
};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodFilter, on};
use axum::{Json, Router};
use chrono::Utc;
use clickman_protocol::{Limits, Sanitizer, normalize};
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Instant;
use time::OffsetDateTime;
use tower_http::cors::{Any, CorsLayer};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

/// Method/path declarations used by the embedded router and host CLI tools.
pub const HTTP_ROUTES: [(Method, &str); 4] = [
    (Method::POST, "/v1/batch"),
    (Method::POST, "/graphql"),
    (Method::GET, "/health"),
    (Method::POST, "/v1/identify"),
];

/// Proxy headers are only accepted when the operator trusts a proxy which
/// overwrites them. Invalid forwarded addresses fall back to the socket peer.
pub fn request_ip(
    headers: &axum::http::HeaderMap,
    peer: Option<IpAddr>,
    trust_proxy: bool,
) -> IpAddr {
    let forwarded = trust_proxy
        .then(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .and_then(|v| v.trim().parse::<IpAddr>().ok())
        })
        .flatten();
    forwarded
        .or(peer)
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

pub fn router(engine: Engine) -> Router {
    let [
        (batch_method, batch_path),
        (graphql_method, graphql_path),
        (health_method, health_path),
        (identify_method, identify_path),
    ] = HTTP_ROUTES.map(|(method, path)| {
        (
            MethodFilter::try_from(method).expect("standard HTTP method"),
            path,
        )
    });
    let public = Router::new()
        .route(batch_path, on(batch_method, batch))
        .route(identify_path, on(identify_method, identify))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::POST])
                .allow_headers([
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::CONTENT_ENCODING,
                ]),
        );
    Router::new()
        .merge(public)
        .route(graphql_path, on(graphql_method, graphql))
        .route(health_path, on(health_method, health))
        .layer(DefaultBodyLimit::max(MAX_WIRE_BYTES))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(engine)
}

async fn health(State(engine): State<Engine>) -> Response {
    if engine.storage.healthy().await {
        (StatusCode::OK, "ok").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable").into_response()
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdentityLink {
    anonymous_id: String,
    external_id: String,
}
async fn identify(State(engine): State<Engine>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let Some(token) = bearer(&parts.headers) else {
        return failure(StatusCode::UNAUTHORIZED, "invalid_write_key");
    };
    let identity = match engine.storage.write_identity(token).await {
        Ok(Some(i)) => i,
        Ok(None) => return failure(StatusCode::UNAUTHORIZED, "invalid_write_key"),
        Err(_) => return retry(failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"), 5),
    };
    if !identity.settings.accepts_origin(
        parts
            .headers
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok()),
    ) {
        return failure(StatusCode::FORBIDDEN, "origin_not_allowed");
    }
    let peer = request_ip(
        &parts.headers,
        parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|p| p.0.ip()),
        engine.trust_proxy_headers,
    );
    if let Decision::Limited { retry_after } = engine.limiter.check(
        &format!("{}:identity:{peer}", identity.key_id),
        identity.settings.rate_limit,
        Instant::now(),
    ) {
        return retry(
            failure(StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            retry_after.as_secs(),
        );
    }
    let Ok(bytes) = axum::body::to_bytes(body, 4096).await else {
        return failure(StatusCode::PAYLOAD_TOO_LARGE, "identity_too_large");
    };
    let Ok(link) = serde_json::from_slice::<IdentityLink>(&bytes) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_identity");
    };
    match engine
        .storage
        .link_identity(&identity, &link.anonymous_id, &link.external_id, Utc::now())
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(clickman_storage::Error::Invalid(message)) => {
            if message == "write key revoked" {
                failure(StatusCode::UNAUTHORIZED, "invalid_write_key")
            } else {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({"error":"invalid_identity","message":message})),
                )
                    .into_response()
            }
        }
        Err(_) => retry(failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"), 5),
    }
}
async fn graphql(
    State(engine): State<Engine>,
    headers: axum::http::HeaderMap,
    Json(request): Json<async_graphql::Request>,
) -> Response {
    let access = match engine.authenticator.authenticate(&headers).await {
        Ok(Some(access)) => access,
        Ok(None) => return failure(StatusCode::UNAUTHORIZED, "unauthorized"),
        Err(error) => {
            tracing::error!(%error,"authentication storage failed");
            return retry(failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"), 5);
        }
    };
    let response = Json(engine.schema.execute(request.data(access)).await).into_response();
    let mut response = response;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

async fn batch(State(engine): State<Engine>, request: Request) -> Response {
    let received_at = OffsetDateTime::now_utc();
    let (parts, body) = request.into_parts();
    let Some(token) = bearer(&parts.headers) else {
        return failure(StatusCode::UNAUTHORIZED, "invalid_write_key");
    };
    let identity = match engine.storage.write_identity(token).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return failure(StatusCode::UNAUTHORIZED, "invalid_write_key"),
        Err(error) => {
            tracing::error!(%error,"write key lookup failed");
            return retry(failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"), 5);
        }
    };
    if !identity.settings.accepts_origin(
        parts
            .headers
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok()),
    ) {
        return failure(StatusCode::FORBIDDEN, "origin_not_allowed");
    }
    let address = request_ip(
        &parts.headers,
        parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|p| p.0.ip()),
        engine.trust_proxy_headers,
    );
    let bucket = format!("{}:{address}", identity.key_id);
    if let Decision::Limited { retry_after } =
        engine
            .limiter
            .check(&bucket, identity.settings.rate_limit, Instant::now())
    {
        return retry(
            failure(StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            retry_after.as_secs(),
        );
    }
    let Ok(bytes) = axum::body::to_bytes(body, MAX_WIRE_BYTES).await else {
        return failure(StatusCode::PAYLOAD_TOO_LARGE, "batch_too_large");
    };
    let encoding = parts
        .headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok());
    let decoded = match body::decode(encoding, &bytes) {
        Ok(decoded) => decoded,
        Err(BodyError::TooLarge) => {
            return failure(StatusCode::PAYLOAD_TOO_LARGE, "batch_too_large");
        }
        Err(BodyError::UnsupportedEncoding) => {
            return failure(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_encoding");
        }
        Err(BodyError::Malformed(message)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"malformed_batch","message":message})),
            )
                .into_response();
        }
    };
    let limits = Limits {
        max_event_age: time::Duration::days(identity.settings.max_event_age_days.into()),
        ..Limits::default()
    };
    let sanitizer = Sanitizer::new(&identity.settings.filter_fragments);
    let normalized = match normalize(&decoded, received_at, &sanitizer, &limits) {
        Ok(normalized) => normalized,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"malformed_batch","message":error.to_string()})),
            )
                .into_response();
        }
    };
    match engine.storage.insert(&identity,&normalized.events,Utc::now()).await {
        Ok(inserted)=>(StatusCode::ACCEPTED,Json(json!({"accepted":inserted,"duplicates":normalized.events.len() as u64-inserted,"rejected":normalized.rejected}))).into_response(),
        Err(clickman_storage::Error::Invalid(_))=>failure(StatusCode::UNAUTHORIZED,"invalid_write_key"),
        Err(clickman_storage::Error::Capacity{retry_after_seconds})=>retry(failure(StatusCode::TOO_MANY_REQUESTS,"capacity_exhausted"),retry_after_seconds),
        Err(error)=>{tracing::error!(project=%identity.project_id,%error,"batch storage failed");retry(failure(StatusCode::SERVICE_UNAVAILABLE,"unavailable"),5)}
    }
}

fn failure(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"error":error}))).into_response()
}
fn retry(mut response: Response, seconds: u64) -> Response {
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, seconds.to_string().parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarded_addresses_require_operator_trust_and_valid_ip_syntax() {
        let peer = "127.0.0.1".parse().unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "2001:db8::42, 127.0.0.1".parse().unwrap(),
        );
        assert_eq!(request_ip(&headers, Some(peer), false), peer);
        assert_eq!(
            request_ip(&headers, Some(peer), true),
            "2001:db8::42".parse::<IpAddr>().unwrap()
        );
        headers.insert(
            "x-forwarded-for",
            "arbitrary-limiter-bucket".parse().unwrap(),
        );
        assert_eq!(request_ip(&headers, Some(peer), true), peer);
    }
}
