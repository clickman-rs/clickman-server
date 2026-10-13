use axum::{Json, Router, http::HeaderMap, routing::post};
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_clickman-mcp-server"));
    for name in [
        "CLICKMAN_MCP_GRAPHQL_ENDPOINT",
        "CLICKMAN_MCP_TOKEN",
        "CLICKMAN_MCP_COOKIE",
        "CLICKMAN_MCP_ORIGIN",
        "CLICKMAN_MCP_ALLOW_WRITES",
    ] {
        command.env_remove(name);
    }
    command
}

#[tokio::test]
async fn actual_stdio_binary_negotiates_and_calls_the_authorized_http_host() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let router = Router::new().route(
        "/graphql",
        post(|headers: HeaderMap, Json(input): Json<Value>| async move {
            assert_eq!(headers["authorization"], "Bearer stdio-test-token");
            assert!(
                input["query"]
                    .as_str()
                    .unwrap()
                    .contains("ClickmanMcpProjects")
            );
            Json(json!({"data":{"viewer":{"canCreateProjects":false},"projects":{"edges":[]}}}))
        }),
    );
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut command = command();
    command
        .env("CLICKMAN_MCP_GRAPHQL_ENDPOINT", &endpoint)
        .env("CLICKMAN_MCP_TOKEN", "stdio-test-token");
    let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
    assert_eq!(client.list_all_tools().await.unwrap().len(), 8);
    let result = client
        .call_tool(CallToolRequestParams::new("list_projects"))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content.unwrap()["viewer"]["canCreateProjects"],
        false
    );
    client.cancel().await.unwrap();
    http.abort();
}

#[tokio::test]
async fn invalid_configuration_and_help_keep_stdout_reserved_for_protocol() {
    let output = command().arg("--help").output().await.unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CLICKMAN_MCP_GRAPHQL_ENDPOINT"));
    let output = command()
        .env(
            "CLICKMAN_MCP_GRAPHQL_ENDPOINT",
            "https://user:configuration-secret@example.com/graphql",
        )
        .env("CLICKMAN_MCP_TOKEN", "configuration-secret")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("configuration-secret"));
}

#[tokio::test]
async fn oversized_stdio_frames_close_without_buffering_an_unbounded_line() {
    let mut child = command()
        .env(
            "CLICKMAN_MCP_GRAPHQL_ENDPOINT",
            "http://127.0.0.1:1/graphql",
        )
        .env("CLICKMAN_MCP_TOKEN", "frame-limit-test-token")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    // No newline or EOF: the codec itself must reject the line as soon as its
    // limit is crossed, before waiting for more bytes from an untrusted client.
    let _ = input.write_all(&vec![b' '; 96 * 1024]).await;
    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .expect("oversized frame did not close stdio")
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("frame-limit-test-token"));
}
