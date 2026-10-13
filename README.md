# ClickMan server

ClickMan is a self-hosted analytics server backed by PostgreSQL. It receives
events from the Swift, Kotlin, and Rust SDKs, separates data by project, and
provides a dashboard for activity, retention, funnels, and event breakdowns.

Run the API and background worker as separate processes, or embed the engine
in an existing Axum application. The dashboard is a static Svelte application;
Node is needed to build it, rather than to serve production requests.

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

## Embed in your application

The engine starts no listener, scheduler, or migrations implicitly. Your host
supplies authentication, runs maintenance, and chooses its transaction boundary.
The [embedding guide](docs/EMBEDDING.md) includes mounting examples and explains
how to use a host-owned SeaORM transaction.

Try the [runnable example](crates/engine/examples/embedded.rs) with the same
`DATABASE_URL` and `CLICKMAN_ADMIN_TOKEN`:

```sh
cargo run --locked -p clickman-engine --example embedded
```

The example serves its dashboard at `http://127.0.0.1:4131/analytics/`. Keep the
trailing slash on both the Axum mount and the dashboard URL so relative assets
and GraphQL requests resolve correctly.

## Documentation

| Guide | Use it for |
| --- | --- |
| [Wire protocol](docs/PROTOCOL.md) | Event fields, normalization, sanitization, limits, and retries. |
| [Embedding](docs/EMBEDDING.md) | Host authentication, routing, tracking, scheduling, and transactions. |
| [Storage and jobs](docs/STORAGE.md) | Project access, compression, reports, retention, erasure, and destination delivery. |
| [Protocol fixtures](protocol/README.md) | The canonical contract shared by the three SDKs. |

## Workspace

| Path | Responsibility |
| --- | --- |
| `crates/protocol` | v1 parsing, flattening, limits and sanitization |
| `crates/analytics` | column codec, activity/retention calculation and funnels |
| `crates/storage` | SeaORM PostgreSQL access, project management and durable state |
| `crates/destinations` | delivery interface and Mixpanel adapter |
| `crates/jobs` | rotation, retention, reports and delivery orchestration |
| `crates/graphql` | async-graphql schema and project authorization |
| `crates/engine` | embeddable Axum router, authentication and trusted tracking |
| `apps/api`, `apps/worker` | deployable Rust binaries |
| `apps/web` | Svelte dashboard |
| `packages/ui`, `packages/tailwind-config` | shared components and theme |
| `packages/graphql-types` | generated schema/operation types and documents |
| `schema/schema.graphql` | generated backend/frontend contract |
| `migrations` | versioned SeaORM migration crate and PostgreSQL schema |
| `deploy` | Docker image and PostgreSQL/API/worker Compose definition |

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
pnpm check
pnpm build
createdb clickman_test
TEST_DATABASE_URL=postgres://localhost/clickman_test scripts/test-postgres
TEST_DATABASE_URL=postgres://localhost/clickman_test scripts/test-clients
```

The PostgreSQL suite verifies tenant isolation, key revocation, two-worker rotation, partial-chunk topping, sparse-property chunk splitting, late commits, transaction rollback, erasure, retention, funnel filters, durable delivery retry and router embedding. Those tests are explicitly ignored in the database-free suite and enabled by `test-postgres`. `test-clients` builds the three SDK E2E drivers from sibling repositories (`CLICKMAN_SDK_ROOT` overrides their parent), starts its own API, verifies live/offline/restart histories and checks the resulting reports. Use a disposable database; integration projects remain available there for inspection.

Browser tests exercise project creation, source-key issuance, settings, live ingest/report rendering, funnel configuration, project switching, reader permissions, invalid tokens, sign-out and mobile layout:

```sh
pnpm --filter @clickman/web exec playwright install chromium
CLICKMAN_TEST_URL=http://127.0.0.1:4132 \
CLICKMAN_TEST_ADMIN_TOKEN="$CLICKMAN_ADMIN_TOKEN" \
pnpm --filter @clickman/web test:e2e
```

The same suite works against the embedding example with `CLICKMAN_TEST_URL=http://127.0.0.1:4131/analytics/`. Set `PLAYWRIGHT_CHROME=1` to use an installed Google Chrome instead of downloading Chromium.

## Deploy

Build the image with `docker build -f deploy/Dockerfile -t clickman-server:local .`. Copy `deploy/.env.example` to `deploy/.env`, set the database password/URL and instance token, then run `docker compose --env-file deploy/.env -f deploy/compose.yaml up --build -d`. Add project destination credential variables to the worker environment when enabling Mixpanel. The published port binds to loopback for a host-owned HTTPS reverse proxy.

The Docker image contains the API, worker, and built dashboard. The Compose file
also starts PostgreSQL and binds the published API port to loopback. Put your
HTTPS reverse proxy in front of it; keep deployment credentials outside Git.

## License

[MIT](LICENSE).
