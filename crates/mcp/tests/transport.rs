use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use clickman_mcp::{ClientConfig, HttpGraphqlClient, MAX_RESPONSE_BYTES, McpServer};
use rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::task::JoinHandle;

const TOKEN: &str = "test-mcp-access-token-not-a-real-credential";
const PROJECT: &str = "f47ac10b-58cc-4372-a567-0e02b2c3d479";

#[derive(Clone)]
struct Fixture {
    status: StatusCode,
    body: Value,
    requests: Arc<AtomicUsize>,
    session: bool,
}

async fn upstream(
    State(fixture): State<Fixture>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    fixture.requests.fetch_add(1, Ordering::SeqCst);
    if fixture.session {
        assert_eq!(headers["cookie"], "id=test-session-value");
        assert_eq!(headers["origin"], "https://cloud.example.com");
        assert!(!headers.contains_key("authorization"));
    } else {
        assert_eq!(headers["authorization"], format!("Bearer {TOKEN}"));
    }
    let query = body["query"].as_str().unwrap();
    assert!(query.contains("ClickmanMcp"));
    assert!(!query.contains("issueAccessKey"));
    assert!(!query.contains("issueWriteKey"));
    assert!(!query.contains("webhooks"));
    if query.contains("ClickmanMcpSaveSegment") {
        assert!(query.contains("fireExisting: false"));
        assert_eq!(body["variables"]["enabled"], false);
    }
    let mut response = (fixture.status, Json(fixture.body)).into_response();
    if fixture.status == StatusCode::TEMPORARY_REDIRECT {
        response
            .headers_mut()
            .insert("location", "/redirected".parse().unwrap());
    }
    response
}

async fn fixture(
    body: Value,
    status: StatusCode,
    session: bool,
    writes: bool,
) -> (
    RunningService<RoleClient, ()>,
    JoinHandle<()>,
    JoinHandle<()>,
    Arc<AtomicUsize>,
) {
    let requests = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/graphql", post(upstream))
        .route("/redirected", post(upstream))
        .with_state(Fixture {
            status,
            body,
            requests: requests.clone(),
            session,
        });
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let config = ClientConfig::new(&endpoint).unwrap();
    let config = if session {
        config
            .with_session("id=test-session-value", "https://cloud.example.com")
            .unwrap()
    } else {
        config.with_token(TOKEN).unwrap()
    };
    let server = McpServer::new(HttpGraphqlClient::new(config).unwrap(), writes);
    let (server_io, client_io) = tokio::io::duplex(4096);
    let mcp = tokio::spawn(async move {
        server
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = ().serve(client_io).await.unwrap();
    (client, mcp, http, requests)
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &str,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(tool.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[test]
fn endpoints_and_credentials_are_operator_configuration() {
    for endpoint in [
        "http://example.com/graphql",
        "https://user:pass@example.com/graphql",
        "https://example.com/graphql?token=secret",
        "https://example.com/graphql#secret",
        "file:///tmp/token",
    ] {
        assert!(ClientConfig::new(endpoint).is_err());
    }
    for endpoint in [
        "http://127.0.0.1:4132/graphql",
        "http://[::1]:4132/graphql",
        "https://analytics.example.com/analytics/graphql",
    ] {
        assert!(ClientConfig::new(endpoint).is_ok());
    }
    assert!(
        HttpGraphqlClient::new(ClientConfig::new("https://example.com/graphql").unwrap()).is_err()
    );
    assert!(
        ClientConfig::new("https://example.com/graphql")
            .unwrap()
            .with_token("secret\nheader")
            .is_err()
    );
    assert!(
        ClientConfig::new("https://example.com/graphql")
            .unwrap()
            .with_session("id=secret", "https://example.com/path")
            .is_err()
    );
    assert!(
        ClientConfig::new("https://example.com/graphql")
            .unwrap()
            .with_token(TOKEN)
            .unwrap()
            .with_session("id=secret", "https://example.com")
            .is_err()
    );
}

#[tokio::test]
async fn protocol_negotiation_exposes_only_read_tools_without_opt_in() {
    let (client, mcp, http, requests) = fixture(
        json!({"data":{"projects":{"edges":[]}}}),
        StatusCode::OK,
        false,
        false,
    )
    .await;
    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, "clickman-mcp");
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 8);
    assert!(
        tools
            .iter()
            .all(|tool| tool.annotations.as_ref().unwrap().read_only_hint == Some(true))
    );
    let names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    let response = call(&client, "list_projects", json!({})).await;
    assert_eq!(response.is_error, Some(false));
    assert_eq!(
        response.structured_content.unwrap()["projects"]["edges"],
        json!([])
    );
    let denied = client
        .call_tool(
            CallToolRequestParams::new("save_funnel").with_arguments(
                json!({"project_id":PROJECT,"definition":{}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert!(denied.is_err() || denied.is_ok_and(|response| response.is_error == Some(true)));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
}

#[tokio::test]
async fn session_headers_and_input_validation_preserve_the_host_boundary() {
    let (client, mcp, http, requests) = fixture(
        json!({"data":{"project":{"id":PROJECT,"reports":[]}}}),
        StatusCode::OK,
        true,
        false,
    )
    .await;
    let response = call(
        &client,
        "get_reports",
        json!({"project_id":PROJECT,"keys":["events"]}),
    )
    .await;
    assert_eq!(response.is_error, Some(false));
    for arguments in [
        json!({"project_id":"not-a-uuid"}),
        json!({"project_id":PROJECT,"keys":vec!["events";33]}),
        json!({"project_id":PROJECT,"keys":["x".repeat(201)]}),
    ] {
        let response = call(&client, "get_reports", arguments).await;
        assert_eq!(response.is_error, Some(true));
        assert_eq!(
            response.structured_content.unwrap()["error"],
            "INVALID_INPUT"
        );
    }
    let response = call(&client, "preview_funnel", json!({"project_id":PROJECT,"definition":{"key":"bad","name":"Bad","steps":[{"event":"only"}]}})).await;
    assert_eq!(response.is_error, Some(true));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
}

#[tokio::test]
async fn credential_values_and_private_graphql_diagnostics_never_reach_tools() {
    let (client, mcp, http, _) = fixture(json!({"data":{"project":{"id":PROJECT,"reports":[{"key":"events","result":{"events":[{"event":"view","note":format!("echo {TOKEN}"),"api_token":"hidden","signing_ref":"VAULT_SECRET"}]}}]}}}), StatusCode::OK, false, false).await;
    let response = call(&client, "get_reports", json!({"project_id":PROJECT})).await;
    let encoded = serde_json::to_string(&response).unwrap();
    assert!(!encoded.contains(TOKEN));
    assert!(!encoded.contains("hidden"));
    assert!(!encoded.contains("VAULT_SECRET"));
    assert!(encoded.contains("[REDACTED]"));
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
    let (client, mcp, http, _) = fixture(json!({"data":{"project":null},"errors":[{"message":format!("database_password={TOKEN}"),"extensions":{"code":"FORBIDDEN","private":"secret"}}]}), StatusCode::OK, false, false).await;
    let response = call(&client, "get_project", json!({"project_id":PROJECT})).await;
    assert_eq!(response.is_error, Some(true));
    assert_eq!(
        response.structured_content.as_ref().unwrap()["error"],
        "FORBIDDEN"
    );
    assert!(!serde_json::to_string(&response).unwrap().contains(TOKEN));
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
}

#[tokio::test]
async fn response_limits_and_http_failures_are_reported_in_band() {
    for (status, body, expected) in [
        (
            StatusCode::UNAUTHORIZED,
            json!({"secret":TOKEN}),
            "UNAUTHORIZED",
        ),
        (StatusCode::TOO_MANY_REQUESTS, json!({}), "RATE_LIMITED"),
        (StatusCode::TEMPORARY_REDIRECT, json!({}), "UNAVAILABLE"),
        (
            StatusCode::OK,
            json!({"data":{"padding":"x".repeat(MAX_RESPONSE_BYTES)}}),
            "RESPONSE_TOO_LARGE",
        ),
        (StatusCode::OK, json!({"data":null}), "INVALID_RESPONSE"),
    ] {
        let (client, mcp, http, requests) = fixture(body, status, false, false).await;
        let response = call(&client, "list_projects", json!({})).await;
        assert_eq!(response.is_error, Some(true));
        assert_eq!(response.structured_content.unwrap()["error"], expected);
        assert_eq!(
            requests.load(Ordering::SeqCst),
            1,
            "credentials followed a redirect"
        );
        client.cancel().await.unwrap();
        mcp.abort();
        http.abort();
    }
}

#[tokio::test]
async fn chunked_http_response_limits_do_not_depend_on_content_length() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let router = Router::new().route(
        "/graphql",
        post(|| async {
            let chunks = [
                Ok::<_, std::convert::Infallible>(vec![b' '; MAX_RESPONSE_BYTES / 2]),
                Ok(vec![b' '; MAX_RESPONSE_BYTES / 2 + 1]),
            ];
            Response::new(axum::body::Body::from_stream(tokio_stream::iter(chunks)))
        }),
    );
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let server = McpServer::new(
        HttpGraphqlClient::new(
            ClientConfig::new(&endpoint)
                .unwrap()
                .with_token(TOKEN)
                .unwrap(),
        )
        .unwrap(),
        false,
    );
    let (server_io, client_io) = tokio::io::duplex(4096);
    let mcp = tokio::spawn(async move {
        server
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = ().serve(client_io).await.unwrap();
    let response = call(&client, "list_projects", json!({})).await;
    assert_eq!(response.is_error, Some(true));
    assert_eq!(
        response.structured_content.unwrap()["error"],
        "RESPONSE_TOO_LARGE"
    );
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
}

#[tokio::test]
async fn opted_in_writes_are_typed_and_never_backfill_segment_hooks() {
    let definition = json!({"key":"buyers","name":"Buyers","rule":{"kind":"event","filter":{"event":"buy"},"range":"30d"}});
    let (client, mcp, http, requests) = fixture(json!({"data":{"project":{"id":PROJECT,"funnels":[]},"saveSegment":{"id":"segment","definition":definition}}}), StatusCode::OK, false, true).await;
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 10);
    let save = tools
        .iter()
        .find(|tool| tool.name == "save_segment")
        .unwrap();
    assert_eq!(
        save.annotations.as_ref().unwrap().read_only_hint,
        Some(false)
    );
    assert_eq!(
        save.annotations.as_ref().unwrap().destructive_hint,
        Some(true)
    );
    let preview = call(
        &client,
        "preview_segment",
        json!({"project_id":PROJECT,"definition":definition}),
    )
    .await;
    assert_eq!(preview.structured_content.unwrap()["persisted"], false);
    let result = call(
        &client,
        "save_segment",
        json!({"project_id":PROJECT,"definition":definition,"enabled":false}),
    )
    .await;
    assert_eq!(result.is_error, Some(false));
    assert_eq!(requests.load(Ordering::SeqCst), 3);
    let oversized = call(
        &client,
        "save_funnel",
        json!({"project_id":PROJECT,"definition":{"padding":"x".repeat(64*1024)}}),
    )
    .await;
    assert_eq!(oversized.is_error, Some(true));
    assert_eq!(requests.load(Ordering::SeqCst), 3);
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
}
