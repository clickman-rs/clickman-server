use chrono::Utc;
use clickman_engine::{Engine, StandaloneAuthenticator, TrackingEvent};
use clickman_mcp::{ClientConfig, HttpGraphqlClient, McpServer};
use clickman_storage::{ProjectSettings, Role, Storage, sql};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult},
    service::RunningService,
};
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::task::JoinHandle;
use uuid::Uuid;

const ROOT: &str = "owned-mcp-root-token-at-least-32-bytes-long";

async fn connect(
    endpoint: &str,
    token: &str,
    writes: bool,
) -> (RunningService<RoleClient, ()>, JoinHandle<()>) {
    let config = ClientConfig::new(endpoint)
        .unwrap()
        .with_token(token)
        .unwrap();
    let server = McpServer::new(HttpGraphqlClient::new(config).unwrap(), writes);
    let (server_io, client_io) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        server
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    (().serve(client_io).await.unwrap(), task)
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &str,
    arguments: Value,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(tool.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

fn successful(result: CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(false), "{:?}", result.content);
    result.structured_content.unwrap()
}

fn denied(result: CallToolResult, code: &str) {
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.structured_content.unwrap()["error"], code);
}

#[tokio::test]
#[ignore = "requires an owned TEST_DATABASE_URL; run scripts/test-platform"]
async fn real_engine_scopes_reads_writes_source_keys_and_live_revocation() {
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 4)
        .await
        .unwrap();
    storage.migrate().await.unwrap();
    let a = storage
        .create_project(&format!("mcp-{}", Uuid::new_v4().simple()), "Allowed")
        .await
        .unwrap();
    let b = storage
        .create_project(&format!("mcp-{}", Uuid::new_v4().simple()), "Other tenant")
        .await
        .unwrap();
    storage
        .update_settings(
            a.id,
            ProjectSettings {
                rotation_lag_seconds: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let reader = storage
        .issue_access_key(a.id, "reader", Role::Reader)
        .await
        .unwrap();
    let admin = storage
        .issue_access_key(a.id, "admin", Role::Admin)
        .await
        .unwrap();
    let source = storage.issue_write_key(a.id, "browser").await.unwrap();
    let engine = Engine::new(storage.clone())
        .unwrap()
        .with_authenticator(Arc::new(
            StandaloneAuthenticator::new(storage.clone(), ROOT).unwrap(),
        ));
    engine
        .track(
            a.id,
            "server",
            TrackingEvent::new("view", "account_opaque", json!({"format":"mp4"})),
        )
        .await
        .unwrap();
    storage.rotate(a.id, Utc::now()).await.unwrap();
    storage.refresh_reports(a.id, Utc::now()).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let http = tokio::spawn(async move {
        axum::serve(listener, engine.router()).await.unwrap();
    });
    let (client, mcp) = connect(&endpoint, &reader.token, true).await;
    let projects = successful(call(&client, "list_projects", json!({})).await);
    assert_eq!(projects["projects"]["edges"].as_array().unwrap().len(), 1);
    assert_eq!(
        projects["projects"]["edges"][0]["node"]["id"],
        a.id.to_string()
    );
    assert_eq!(projects["projects"]["edges"][0]["node"]["canAdmin"], false);
    assert!(!projects.to_string().contains("Other tenant"));
    denied(
        call(&client, "get_project", json!({"project_id":b.id})).await,
        "FORBIDDEN",
    );
    let events = successful(call(&client, "get_event_catalog", json!({"project_id":a.id})).await);
    assert_eq!(events["report"]["result"]["events"][0]["event"], "view");
    assert_eq!(events["report"]["result"]["events"][0]["events"], 1);
    assert!(events["report"]["computedAt"].is_string());
    let reports = successful(
        call(
            &client,
            "get_reports",
            json!({"project_id":a.id,"keys":["events"]}),
        )
        .await,
    );
    assert_eq!(reports["reports"].as_array().unwrap().len(), 1);
    let funnel = json!({"key":"buy","name":"Buy","steps":[{"event":"view"},{"event":"buy","where":{"format":"mp4"}}]});
    let preview = successful(
        call(
            &client,
            "preview_funnel",
            json!({"project_id":a.id,"definition":funnel}),
        )
        .await,
    );
    assert_eq!(preview["persisted"], false);
    denied(
        call(
            &client,
            "save_funnel",
            json!({"project_id":a.id,"definition":funnel}),
        )
        .await,
        "FORBIDDEN",
    );
    assert!(storage.funnels(a.id).await.unwrap().is_empty());
    storage.revoke_access_key(a.id, reader.id).await.unwrap();
    denied(
        call(&client, "get_project", json!({"project_id":a.id})).await,
        "UNAUTHORIZED",
    );
    client.cancel().await.unwrap();
    mcp.abort();

    let (client, mcp) = connect(&endpoint, &admin.token, true).await;
    successful(
        call(
            &client,
            "save_funnel",
            json!({"project_id":a.id,"definition":funnel}),
        )
        .await,
    );
    let funnels = successful(call(&client, "get_funnels", json!({"project_id":a.id})).await);
    assert_eq!(funnels["funnels"][0]["key"], "buy");
    let segment = json!({"key":"dropoff","name":"Drop-off","rule":{"kind":"funnel","funnel_key":"buy","at_step":1,"exclude_later":true,"wait":"24h"}});
    successful(
        call(
            &client,
            "preview_segment",
            json!({"project_id":a.id,"definition":segment}),
        )
        .await,
    );
    successful(
        call(
            &client,
            "save_segment",
            json!({"project_id":a.id,"definition":segment,"enabled":false}),
        )
        .await,
    );
    let segments = successful(call(&client, "get_segments", json!({"project_id":a.id})).await);
    assert_eq!(segments["segments"][0]["enabled"], false);
    assert_eq!(segments["segments"][0]["fireExisting"], false);
    denied(
        call(
            &client,
            "save_funnel",
            json!({"project_id":b.id,"definition":funnel}),
        )
        .await,
        "FORBIDDEN",
    );
    assert!(storage.funnels(b.id).await.unwrap().is_empty());
    client.cancel().await.unwrap();
    mcp.abort();

    let (client, mcp) = connect(&endpoint, &source.token, true).await;
    denied(
        call(&client, "list_projects", json!({})).await,
        "UNAUTHORIZED",
    );
    client.cancel().await.unwrap();
    mcp.abort();
    http.abort();
    storage
        .connection()
        .execute_raw(sql(
            "DELETE FROM clickman_projects WHERE id IN ($1,$2)",
            vec![a.id.into(), b.id.into()],
        ))
        .await
        .unwrap();
    storage.close().await.unwrap();
}
