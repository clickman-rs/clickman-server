use anyhow::{Context, Result, bail};
use clickman_mcp::{ClientConfig, HttpGraphqlClient, MAX_REQUEST_BYTES, McpServer};
use futures_util::{SinkExt, StreamExt};
use rmcp::{
    RoleServer, ServiceExt,
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::async_rw::JsonRpcMessageCodec,
};
use tokio_util::codec::{FramedRead, FramedWrite};

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(argument) = std::env::args().nth(1) {
        if matches!(argument.as_str(), "--help" | "-h") {
            eprintln!(
                "Clickman MCP (stdio)\n\nRequired: CLICKMAN_MCP_GRAPHQL_ENDPOINT\nAuthentication: CLICKMAN_MCP_TOKEN, or CLICKMAN_MCP_COOKIE + CLICKMAN_MCP_ORIGIN\nOptional: CLICKMAN_MCP_ALLOW_WRITES=1 (funnel/segment save tools)\n\nCredentials are read from the environment, never command-line arguments. See docs/MCP.md."
            );
            return Ok(());
        }
        bail!("usage: clickman-mcp-server [--help]");
    }
    // stdout belongs exclusively to MCP JSON-RPC. Diagnostics use stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("warn")
        .init();
    let endpoint = optional_environment("CLICKMAN_MCP_GRAPHQL_ENDPOINT")?
        .context("CLICKMAN_MCP_GRAPHQL_ENDPOINT is required")?;
    let config = ClientConfig::new(&endpoint)?;
    let config = match (
        optional_environment("CLICKMAN_MCP_TOKEN")?,
        optional_environment("CLICKMAN_MCP_COOKIE")?,
        optional_environment("CLICKMAN_MCP_ORIGIN")?,
    ) {
        (Some(token), None, None) => config.with_token(&token)?,
        (None, Some(cookie), Some(origin)) => config.with_session(&cookie, &origin)?,
        _ => bail!(
            "Configure CLICKMAN_MCP_TOKEN, or both CLICKMAN_MCP_COOKIE and CLICKMAN_MCP_ORIGIN."
        ),
    };
    let allow_writes = match std::env::var("CLICKMAN_MCP_ALLOW_WRITES").as_deref() {
        Ok("1") => true,
        Ok("0") | Err(_) => false,
        _ => bail!("CLICKMAN_MCP_ALLOW_WRITES must be 0 or 1."),
    };
    // Bound a full JSON-RPC line before deserialization. Invalid or oversized
    // frames close the transport; do not keep buffering attacker-controlled I/O.
    let input = FramedRead::new(
        tokio::io::stdin(),
        JsonRpcMessageCodec::<RxJsonRpcMessage<RoleServer>>::new_with_max_length(MAX_REQUEST_BYTES),
    )
    .take_while(|item| std::future::ready(item.is_ok()))
    .map(|item| item.expect("take_while retained only valid frames"));
    let output = FramedWrite::new(
        tokio::io::stdout(),
        JsonRpcMessageCodec::<TxJsonRpcMessage<RoleServer>>::default(),
    )
    .sink_map_err(std::io::Error::from);
    let server = McpServer::new(HttpGraphqlClient::new(config)?, allow_writes)
        .serve((output, input))
        .await?;
    server.waiting().await?;
    Ok(())
}

fn optional_environment(name: &'static str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok((!value.is_empty()).then_some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{name} must be valid UTF-8"),
    }
}
