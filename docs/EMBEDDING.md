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

Use `engine.router_with_dashboard("apps/web/dist")` to include the built dashboard. Mount it with `Router::new().nest("/analytics/", engine.router_with_dashboard("apps/web/dist"))` and open `/analytics/`; Axum's mount and the dashboard URL both need the trailing slash so assets and GraphQL resolve relative to the mount. `VITE_CLICKMAN_GRAPHQL_ENDPOINT` can override the dashboard endpoint at build time.

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
