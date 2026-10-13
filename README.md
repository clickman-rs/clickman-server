# ClickMan server

ClickMan is an open-source analytics engine with PostgreSQL and ClickHouse
analytics adapters. PostgreSQL holds transactional project metadata and the
durable ingestion inbox in both modes. It receives browser, Swift, Kotlin and
Rust events and provides activity, retention, funnels, segments and webhooks.

Run the API and background worker as separate processes, or embed the engine
in an existing Axum application. The standalone dashboard is a SvelteKit static
SPA consuming the reusable `@clickman/dashboard` Svelte package. Node is needed
to build it; the Rust server serves production assets.

## Run locally

You need the pinned Rust toolchain, Node 22, pnpm 10.29.2, PostgreSQL 14 or later,
and Python 3 for the token-generation command below. The Rust version is selected
by [rust-toolchain.toml](rust-toolchain.toml).

From the repository root, create a database, build the dashboard, and start the API:

```sh
createdb clickman
export DATABASE_URL=postgres://localhost/clickman
export CLICKMAN_ADMIN_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(48))')"
pnpm install --frozen-lockfile
pnpm build
cargo run --locked -p clickman-api
```

Open `http://127.0.0.1:4132`, sign in with `CLICKMAN_ADMIN_TOKEN`, and create a
project. In Settings, issue a source write key for each SDK integration. The API
applies database migrations before serving requests.

In another terminal, set the same `DATABASE_URL` and start the worker:

```sh
export DATABASE_URL=postgres://localhost/clickman
cargo run --locked -p clickman-worker
```

The worker starts processing immediately and repeats every minute. It rotates
events into compressed storage, applies retention, refreshes reports, and sends
pending destination pages. It does not need the admin token. Defaults include a
two-minute processing delay and a seven-day raw-event deduplication window.
Segments are evaluated on every tick, including timed conditions with no new
events; signed webhook delivery and retry state survive a worker restart.

[.env.example](.env.example) lists process configuration. Set those variables in
the process environment; the file is a reference template. `clickman-api migrate`
applies migrations without serving HTTP, and `clickman-api schema` prints the
GraphQL contract without a database connection.

## Connect an SDK

The server accepts the [v1 batch protocol](docs/PROTOCOL.md) at `POST /v1/batch`.
A source write key chooses the project and authorizes ingestion only; it cannot
open the dashboard or GraphQL API.

Use the server's base URL as the SDK endpoint. For a standalone deployment, that
might be `https://analytics.example.com`; for an embedded engine, it might be
`https://example.com/analytics`. The client appends `/v1/batch`.

The separate `clickman-swift`, `clickman-kotlin`, and `clickman-rust` repositories
contain the native clients and their local protocol fixtures.
The `clickman-js` repository contains the browser SDK with consent control,
IndexedDB persistence and anonymous visitor linking. Its packed archive and
browser bundles can be distributed directly; npm publication is deferred.

## Embed in your application

The engine starts no listener, scheduler, or migrations implicitly. Your host
supplies authentication, runs maintenance, and chooses its transaction boundary.
The [embedding guide](docs/EMBEDDING.md) includes mounting examples and explains
how to use a host-owned SeaORM transaction.

Try the [runnable example](crates/engine/examples/embedded.rs) with the same
`DATABASE_URL` and `CLICKMAN_ADMIN_TOKEN`:

```sh
CLICKMAN_BASE_PATH=/analytics pnpm build
cargo run --locked -p clickman-engine --example embedded
```

The example serves its dashboard at `http://127.0.0.1:4131/analytics/`. Keep the
trailing slash on the Axum mount and dashboard entry URL. Build-time
`CLICKMAN_BASE_PATH` sets the asset, routing and default GraphQL prefix. Direct
links to `/analytics/projects/<id>/retention` work through the static fallback.
Rebuild without that variable to serve the standalone dashboard at the root.

Svelte hosts can instead import [the dashboard package](packages/dashboard/README.md)
and supply their own Cachebay client and navigation callback. Ferment uses this
mode with its existing session-authorized analytics endpoint; its admin shell
owns authentication and routing.

## Documentation

| Guide                                             | Use it for                                                                          |
| ------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [Wire protocol](docs/PROTOCOL.md)                 | Event fields, normalization, sanitization, limits, and retries.                     |
| [Embedding](docs/EMBEDDING.md)                    | Host authentication, routing, tracking, scheduling, and transactions.               |
| [Storage and jobs](docs/STORAGE.md)               | Project access, compression, reports, retention, erasure, and destination delivery. |
| [Protocol fixtures](protocol/README.md)           | The canonical contract shared by the three SDKs.                                    |
| [Dashboard package](packages/dashboard/README.md) | Public Svelte API, client ownership, styles and release checks.                     |
| [Platform architecture](docs/PLATFORM.md)         | Adapters, browser identities, segments, webhooks and SaaS boundaries.               |
| [MCP server](docs/MCP.md)                         | Authenticated stdio tools for reports, funnels and segments.                        |

## Workspace

| Path                      | Responsibility                                                                           |
| ------------------------- | ---------------------------------------------------------------------------------------- |
| `crates/protocol`         | v1 parsing, flattening, limits and sanitization                                          |
| `crates/analytics`        | column codec, activity/retention calculation and funnels                                 |
| `crates/storage`          | PostgreSQL metadata/inbox, analytics adapters, identity/segment/delivery state           |
| `crates/destinations`     | delivery interface, Mixpanel adapter and signed webhook sender                           |
| `crates/jobs`             | rotation, retention, reports and delivery orchestration                                  |
| `crates/graphql`          | async-graphql schema and project authorization                                           |
| `crates/engine`           | embeddable Axum router, authentication and trusted tracking                              |
| `crates/mcp`, `apps/mcp`  | typed MCP tools and stdio executable over an existing authenticated GraphQL host         |
| `apps/api`, `apps/worker` | deployable Rust binaries                                                                 |
| `apps/web`                | SvelteKit static standalone host: auth, client, layout and routes                        |
| `packages/dashboard`      | shared Svelte workspace, internal components, operations/types and compiled Tailwind CSS |
| `schema/schema.graphql`   | generated backend/frontend contract                                                      |
| `migrations`              | versioned SeaORM migration crate and PostgreSQL schema                                   |
| `deploy`                  | Docker image and PostgreSQL/API/worker Compose definition                                |

`pnpm generate` re-exports the Rust schema and regenerates TypeScript documents/types. Commit both outputs when the contract changes. This command does not need PostgreSQL. SDK queue fixtures are a separate versioned wire contract in `protocol/`.

## Develop and test

Run `pnpm dev` alongside the API for frontend development. Vite proxies
`/graphql` to port 4132; set `CLICKMAN_PROXY_TARGET` for another API address.
The production API serves `apps/web/dist`; `VITE_CLICKMAN_GRAPHQL_ENDPOINT`
can override the dashboard endpoint at build time.

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
scripts/test-platform --clickhouse
pnpm check
pnpm format:check
pnpm --filter @clickman/dashboard test
pnpm --filter @clickman/web test:unit
pnpm test:package
pnpm build
createdb clickman_test
TEST_DATABASE_URL=postgres://localhost/clickman_test scripts/test-postgres
TEST_DATABASE_URL=postgres://localhost/clickman_test scripts/test-clients
```

The PostgreSQL suite verifies tenant isolation, key revocation, two-worker rotation, partial-chunk topping, sparse-property chunk splitting, late commits, transaction rollback, erasure, retention, funnel filters, durable delivery retry and router embedding. Those tests are explicitly ignored in the database-free suite and enabled by `test-postgres`. `test-clients` builds the three SDK E2E drivers from sibling repositories (`CLICKMAN_SDK_ROOT` overrides their parent), starts its own API, verifies live/offline/restart histories and checks the resulting reports. Use a disposable database; integration projects remain available there for inspection.

Browser tests exercise project creation, source-key issuance, settings, live
ingest/reports, funnels, direct links/history, delayed project/session responses,
reader permissions, invalid tokens, locked mutation drafts and mobile layout.
The runner owns and removes a disposable PostgreSQL cluster and chooses unused
API/database ports; it does not read a development database URL:

```sh
pnpm --filter @clickman/web exec playwright install chromium
pnpm test:browser
pnpm test:browser --embedded
```

Set `CLICKMAN_POSTGRES_BIN` to a PostgreSQL server bin directory if it is not on
PATH. Set `PLAYWRIGHT_CHROME=1` to use installed Google Chrome. To target your
own disposable test server, run `pnpm --filter @clickman/web test:e2e` with
`CLICKMAN_TEST_URL` (including any prefix/trailing slash) and
`CLICKMAN_TEST_ADMIN_TOKEN`.

`scripts/test-platform` creates and removes its own PostgreSQL cluster. With
`--clickhouse`, it also owns a ClickHouse 26.9 container and runs both adapters'
conformance tests. Docker is required for that mode. It never reads the
application `DATABASE_URL`. See [platform acceptance](docs/DASHBOARD.md) for
results and the limits of the local comparison benchmark.

## Deploy

Build the image with `docker build -f deploy/Dockerfile -t clickman-server:local .`. Copy `deploy/.env.example` to `deploy/.env`, set the database password/URL and instance token, then run `docker compose --env-file deploy/.env -f deploy/compose.yaml up --build -d`. Add project destination credential variables to the worker environment when enabling Mixpanel. The published port binds to loopback for a host-owned HTTPS reverse proxy.

The Docker image contains the API, worker, MCP executable and built dashboard. The Compose file
also starts PostgreSQL and binds the published API port to loopback. Put your
HTTPS reverse proxy in front of it; keep deployment credentials outside Git.

## License

[MIT](LICENSE).
