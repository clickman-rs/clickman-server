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
use std::net::SocketAddr;
use std::time::Instant;
use time::OffsetDateTime;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

/// Method/path declarations used by the embedded router and host CLI tools.
pub const HTTP_ROUTES: [(Method, &str); 3] = [
    (Method::POST, "/v1/batch"),
    (Method::POST, "/graphql"),
    (Method::GET, "/health"),
];

pub fn router(engine: Engine) -> Router {
    let [
        (batch_method, batch_path),
        (graphql_method, graphql_path),
        (health_method, health_path),
    ] = HTTP_ROUTES.map(|(method, path)| {
        (
            MethodFilter::try_from(method).expect("standard HTTP method"),
            path,
        )
    });
    Router::new()
        .route(batch_path, on(batch_method, batch))
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
    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    let address = if engine.trust_proxy_headers {
        parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_owned())
            .filter(|v| v.len() <= 128)
            .unwrap_or(peer)
    } else {
        peer
    };
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
