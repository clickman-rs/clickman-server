# Embedding ClickMan in an Axum application

Use the engine when your application already owns authentication, routing, and scheduling. The [standalone quick start](../README.md) needs no host integration. This guide explains mounting, authorization, trusted tracking, and transactions supplied by the host.

## Mount the engine

```rust
let storage = clickman_storage::Storage::connect(database_url, 10).await?;
storage.migrate().await?;
let engine = clickman_engine::Engine::new(storage)?
    .with_authenticator(std::sync::Arc::new(host_authenticator));
let app = axum::Router::new().nest("/analytics", engine.router());
```

The host implements `Authenticator` and returns `Access::project(id, Role::Reader)`, `Access::project(id, Role::Admin)`, a collection of project roles, or instance-level `Access::root()`. Engine construction denies dashboard access until an authenticator is installed. The host owns users, sessions and its application router. The engine starts no listener, scheduler or migrations implicitly. See the [runnable embedding example](../crates/engine/examples/embedded.rs): `cargo run -p clickman-engine --example embedded`.

Use `engine.router_with_dashboard("apps/web/dist")` to include the built
standalone application. Build it for the chosen mount first:

```sh
CLICKMAN_BASE_PATH=/analytics pnpm build
cargo run --locked -p clickman-engine --example embedded
```

Mount with `Router::new().nest("/analytics/", engine.router_with_dashboard("apps/web/dist"))`
and open `/analytics/`. The Axum mount needs the trailing slash. SvelteKit uses
the configured base for absolute assets, client routes and `/analytics/graphql`;
the static fallback also handles direct project URLs. `VITE_CLICKMAN_GRAPHQL_ENDPOINT`
can override that endpoint at build time. The bootstrap is emitted as an external
script so a host CSP can use `script-src 'self'` without allowing inline scripts.
The example accepts `CLICKMAN_BIND`, defaulting to `127.0.0.1:4131`.

## Import the workspace into a Svelte host

Use [the dashboard package](../packages/dashboard/README.md) when your host
already has a Svelte shell. Pass its `ClickmanWorkspace` an authenticated Cachebay
client, a `ClickmanRoute`, and `onnavigate`. The host owns the session, client
disposal, request cancellation and URL mapping. The package supplies the same
reports, funnels and management screens as the standalone application.

Import `@clickman/dashboard/styles.css`; it ships compiled Tailwind styles with
prefixed utilities and scoped defaults, without global Preflight. Host-specific
theme variables are documented in the package README. The component has no
SvelteKit router dependency, so it also works in a plain Svelte application.

Ferment uses `/admin/api/analytics/graphql` through its existing session-owned
analytics client. This executes the same Clickman schema with the host's
transaction and project roles. No iframe, separate token login, new GraphQL
schema or duplicate dashboard is required. Its SDK source keys authenticate
`/analytics/v1/batch`. The current Ferment host does not install an authenticator
on the raw `/analytics/graphql` route: project access keys can be managed through
the staff schema, but Bearer requests to the raw route return 401. To serve the
standalone token-login application from a host, configure its authenticator as
shown above; mounting the router alone does not enable that workflow.

## Jobs and host transactions

Schedule `engine.jobs().tick()` with the host's scheduler, or use `clickman-worker` against the same database. Trusted server-side events go through the same validation and sanitization as device batches:

```rust
let mut event = clickman_engine::TrackingEvent::new(
    "subscription_started", user_id, serde_json::json!({"product": "yearly"}),
);
event.message_id = operation_id; // stable across host retries
engine.track(project_id, "api", event).await?;
```

This method expects the host to authorize `project_id`; the `track` convenience method owns its transaction. A host can instead bind the normal storage operations to an existing SeaORM PostgreSQL transaction:

```rust
use sea_orm::TransactionTrait;

let tx = std::sync::Arc::new(database.begin().await?);
// Authorize and lock the host actor using this same transaction.
let storage = clickman_storage::Storage::from_transaction(tx.clone())?;
let mut response = engine.schema().execute(request.data(storage).data(access)).await;
let tx = std::sync::Arc::try_unwrap(tx)
    .map_err(|_| anyhow::anyhow!("request retained its transaction"))?;
if response.errors.is_empty() {
    tx.commit().await?;
} else {
    tx.rollback().await?;
    response.data = async_graphql::Value::Null;
}
```

The caller owns commit, rollback and database/request deadlines. Request data overrides the engine's normal pool-backed storage. Storage methods that begin transactions use savepoints, so their internal commits cannot publish before the caller commits. Run transaction-opening operations sequentially and drop every scoped storage clone and unfinished operation before unwrapping the `Arc`. Dropping the last uncommitted transaction owner uses SeaORM rollback. `Storage::close` closes a pool-backed storage; it returns an error for a transaction view. `Storage::connection()` exposes `ConnectionTrait` and `TransactionTrait` rather than the concrete pool type, so operations cannot escape the supplied transaction.
