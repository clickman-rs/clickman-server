# Clickman MCP

Clickman includes a local stdio Model Context Protocol server for MCP clients.
`crates/mcp` owns the reusable tools and authenticated GraphQL client;
`apps/mcp` owns the executable. It uses the
[official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
(`rmcp` 3.5.1), including negotiation and generated tool input schemas.
There is no separate npm package, database credential, public MCP listener or
model-provider dependency.

Every remote tool uses an existing GraphQL endpoint. Standalone checks the
current access token on every request. Clickman Cloud checks its existing
member session, organization role and project grants. Revocation takes effect
on the next call; a project UUID never grants access itself. The Cloud browser
agent remains a separate browser-side AI SDK integration.

## Build and connect

```sh
cargo build --release --locked -p clickman-mcp-server
```

Point the MCP client's stdio command to the absolute path of
`target/release/clickman-mcp-server`. Credentials are read from the private
process environment, never command-line arguments. Supply them through the
MCP client's private environment or secret configuration and keep their values
outside source control and shared screenshots.

Example configuration, with authentication supplied separately by the client's
private environment:

```json
{
  "mcpServers": {
    "clickman": {
      "command": "/absolute/path/clickman-server/target/release/clickman-mcp-server",
      "env": {
        "CLICKMAN_MCP_GRAPHQL_ENDPOINT": "https://analytics.example.com/graphql",
        "CLICKMAN_MCP_ALLOW_WRITES": "0"
      }
    }
  }
}
```

For standalone, privately set `CLICKMAN_MCP_TOKEN` to a project reader/admin
access token from the dashboard. A reader token is sufficient for analysis.
The instance admin token is supported when instance-wide access is intended.
A source write key (`cmw_…`) authorizes ingestion only and cannot be used for
MCP. An embedded prefix changes the endpoint, for example
`https://example.com/analytics/graphql`.

For Clickman Cloud, use these variables instead of `CLICKMAN_MCP_TOKEN`:

| Variable                        | Value                                                                   |
| ------------------------------- | ----------------------------------------------------------------------- |
| `CLICKMAN_MCP_GRAPHQL_ENDPOINT` | `https://cloud.example.com/api/organizations/<organization ID>/graphql` |
| `CLICKMAN_MCP_COOKIE`           | Complete authenticated member `Cookie` header, supplied privately.      |
| `CLICKMAN_MCP_ORIGIN`           | Exact `SAAS_PUBLIC_ORIGIN`, such as `https://cloud.example.com`.        |

Cloud keeps session authentication. MCP does not enable project access-key
login or bypass verification/project grants. Expired/revoked sessions return an
authentication error and must be renewed through normal Cloud login. The
explicit Origin preserves unsafe-request checks for GraphQL POSTs. Do not
configure both token and cookie authentication.

Endpoints require HTTPS; HTTP is accepted only for `localhost`/loopback IPs
during local development. URL user information, query strings and fragments are
rejected. Redirects and environment proxies are disabled. The endpoint, Origin
and authentication are operator configuration; no tool can change them.

## Tools

| Tool                | Behavior                                                                              |
| ------------------- | ------------------------------------------------------------------------------------- |
| `list_projects`     | Accessible project metadata and current administration flags.                         |
| `get_project`       | Name, slug, UUID, creation time and administration flag.                              |
| `get_reports`       | Cached activity, retention, events and funnel reports; optional report-key selection. |
| `get_event_catalog` | Event names and 30-day totals from the latest cached events report.                   |
| `get_funnels`       | Saved funnel definitions.                                                             |
| `get_segments`      | Definitions, enabled state, revision, cached count and evaluation time.               |
| `preview_funnel`    | Validate/normalize a proposal using exact engine rules; check project access.         |
| `preview_segment`   | Validate/normalize a proposal against the project's current funnels.                  |
| `save_funnel`       | Optional: create/replace a funnel by key.                                             |
| `save_segment`      | Optional: create/replace a segment by key and explicit enabled state.                 |

Reports are worker-computed snapshots. Inspect `computedAt` and `evaluatedAt`
before drawing conclusions. An absent events report has not been computed yet.
Previews validate definitions; they do not compute conversion or estimate
membership. Reads and previews never trigger maintenance.

All project tools require `project_id` as a UUID from `list_projects`.
`get_reports.keys` accepts up to 32 keys of 1–200 bytes. Omit it or use an empty
list for all snapshots; specify `events`, `actives`, `retention` or
`funnel:<key>` to filter in PostgreSQL before loading result JSON. The underlying
GraphQL `reports(keys:)` field distinguishes an omitted/null argument (all
reports) from an explicit empty list (no reports).

Write tools are absent from discovery and cannot be called by default. Set
`CLICKMAN_MCP_ALLOW_WRITES=1` explicitly to expose them. The host still requires
current project-admin permissions. The MCP client should show the project and
previewed definition and obtain user approval before invoking a write. Tool
annotations describe risk; they do not replace approval or host authorization.
Parallel writes in one process receive a retryable limit error.

Replacing a funnel invalidates its report and may revise dependent segments.
Segment definition/enabled changes may cancel pending old-rule webhook
deliveries. `save_segment` always sets `fireExisting: false`; MCP never backfills
existing members. Future entries can trigger webhooks already configured in the
dashboard. Key issuance, integration credentials, webhook setup, project
deletion, erasure and maintenance are deliberately not MCP tools.

Names, event labels, property values and report content are untrusted data,
never instructions. Tools select no credentials or integration configuration.
They redact credential-bearing fields and configured token/session values from
returned JSON. Remote messages, HTTP bodies and private GraphQL extensions are
not returned; errors contain a bounded code and safe message.

## Embed and limits

Hosts can construct the same server without environment parsing:

```rust
use clickman_mcp::{ClientConfig, HttpGraphqlClient, McpServer};

let config = ClientConfig::new(graphql_endpoint)?
    .with_session(member_cookie, public_origin)?;
let server = McpServer::new(HttpGraphqlClient::new(config)?, false);
// Serve using an rmcp transport chosen and authenticated by the host.
```

Standalone uses `config.with_token(access_token)` instead. The library never
creates an `Access`, acquires a database connection or commits a host
transaction. The existing GraphQL handler owns authorization and transactions.

The executable reserves stdout for JSON-RPC; diagnostics and `--help` use stderr.
Its codec rejects lines over 64 KiB before deserialization, including lines
without a terminating newline. Malformed/oversized frames close that stdio
connection. GraphQL requests are capped at 64 KiB, definition JSON at 60 KiB,
and responses at 2 MiB, including chunked HTTP. Connect/exchange deadlines are
5/30 seconds with at most eight concurrent remote requests. Segment saves first
validate referenced funnels, then send a separately authorized mutation; each
exchange has that deadline. Tool failures use in-band `isError` results; rmcp
handles invalid protocol requests.

The Docker image also contains `clickman-mcp-server`. A client may start this
binary through Docker with an interactive stdio pipe and private environment.
MCP requires neither `DATABASE_URL` nor a worker.

## Verify

```sh
cargo test --locked -p clickman-mcp --test transport
cargo test --locked -p clickman-mcp-server --test stdio
scripts/test-platform cargo test --locked -p clickman-mcp --test host -- --include-ignored
```

Tests exercise official-SDK negotiation, discovery/annotations, typed
operations, token/cookie/Origin, read-only enforcement, validation, redaction,
HTTP/output limits and the actual stdio executable. The real engine/PostgreSQL
scenario verifies tenant scope, reader/admin writes, source-key rejection,
cached reports and live revocation. Tests own local fixtures; no production
endpoint or provider account is required.
