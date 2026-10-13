use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, Duration, Utc};
use clickman_destinations::{Dispatcher, Error as DeliveryError};
use clickman_engine::{Engine, StandaloneAuthenticator, TrackingEvent};
use clickman_protocol::Event;
use clickman_storage::{Destination, ProjectSettings, Role, Storage, StoredEvent, sql};
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tower::ServiceExt;
use uuid::Uuid;

const ROOT: &str = "clickman-test-root-token-32-bytes-long";

async fn storage() -> Storage {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("set TEST_DATABASE_URL to a disposable PostgreSQL database");
    let storage = Storage::connect(&url, 8).await.unwrap();
    storage.migrate().await.unwrap();
    storage
}
async fn project(storage: &Storage, name: &str) -> Uuid {
    let id = storage
        .create_project(&format!("test-{}", Uuid::new_v4().simple()), name)
        .await
        .unwrap()
        .id;
    storage
        .update_settings(
            id,
            ProjectSettings {
                rotation_lag_seconds: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    id
}
fn engine(storage: Storage) -> Engine {
    Engine::new(storage.clone())
        .unwrap()
        .with_authenticator(Arc::new(
            StandaloneAuthenticator::new(storage, ROOT).unwrap(),
        ))
}
async fn call(router: Router, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .oneshot(
            Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn graphql(
    engine: &Engine,
    token: &str,
    query: &str,
    variables: Value,
) -> (StatusCode, Value) {
    call(
        engine.router(),
        "/graphql",
        token,
        json!({"query":query,"variables":variables}),
    )
    .await
}
fn batch(id: Uuid, external: &str) -> Value {
    let now = Utc::now().to_rfc3339();
    json!({"sentAt":now,"batch":[{"type":"track","messageId":id,"timestamp":now,"event":"export_completed","externalId":external,"properties":{"contact":{"email":"a@b.co"},"format":"mp4"},"context":{"os":{"name":"iOS"}}}]})
}
fn event(id: Uuid, external: &str, name: &str, at: DateTime<Utc>) -> Event {
    Event {
        message_id: id,
        external_id: external.into(),
        event: name.into(),
        occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
            at.timestamp_nanos_opt().unwrap().into(),
        )
        .unwrap(),
        properties: json!({"format":"mp4"}).as_object().unwrap().clone(),
        context: json!({"os.name":"iOS"}).as_object().unwrap().clone(),
    }
}
async fn remove(storage: &Storage, ids: &[Uuid]) {
    for id in ids {
        storage
            .connection()
            .execute_raw(sql(
                "DELETE FROM clickman_projects WHERE id=$1",
                vec![(*id).into()],
            ))
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn sparse_properties_split_chunks_and_continue_after_restart() {
    let storage = storage().await;
    let p = project(&storage, "Sparse properties").await;
    let now = Utc::now();
    let rows: Vec<_> = (0..1500)
        .map(|index| {
            let mut row = event(Uuid::new_v4(), "user", "sparse", now);
            row.context.clear();
            row.properties = serde_json::Map::from_iter([(format!("key_{index}"), json!(true))]);
            row
        })
        .collect();
    storage.insert_server(p, "api", &rows, now).await.unwrap();
    assert_eq!(storage.rotate(p, now).await.unwrap(), 1500);
    let chunks = storage
        .connection()
        .query_all_raw(sql(
            "SELECT events FROM clickman_chunks WHERE project_id=$1 ORDER BY seq",
            vec![p.into()],
        ))
        .await
        .unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].try_get::<i32>("", "events").unwrap(), 1024);
    assert_eq!(chunks[1].try_get::<i32>("", "events").unwrap(), 476);
    let reopened = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 2)
        .await
        .unwrap();
    let last = event(Uuid::new_v4(), "user", "sparse", now);
    reopened
        .insert_server(p, "api", &[last], now)
        .await
        .unwrap();
    assert_eq!(reopened.rotate(p, now).await.unwrap(), 1);
    reopened.refresh_reports(p, now).await.unwrap();
    let reports = reopened.reports(p).await.unwrap();
    let totals = &reports.iter().find(|r| r.key == "events").unwrap().result["events"];
    assert_eq!(totals[0]["events"], 1501);
    assert_eq!(totals[0]["actors"], 1);
    remove(&storage, &[p]).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn project_ingest_dedup_authorization_reports_and_erasure_are_isolated() {
    let storage = storage().await;
    let a = project(&storage, "A").await;
    let b = project(&storage, "B").await;
    let ka = storage.issue_write_key(a, "ios").await.unwrap();
    let kb = storage.issue_write_key(b, "ios").await.unwrap();
    let reader = storage
        .issue_access_key(a, "reader", Role::Reader)
        .await
        .unwrap();
    let admin = storage
        .issue_access_key(a, "admin", Role::Admin)
        .await
        .unwrap();
    let engine = engine(storage.clone());
    let id = Uuid::new_v4();
    assert_eq!(
        call(
            engine.router(),
            "/v1/batch",
            &ka.token,
            batch(id, "same-user")
        )
        .await
        .1["accepted"],
        1
    );
    assert_eq!(
        call(
            engine.router(),
            "/v1/batch",
            &ka.token,
            batch(id, "same-user")
        )
        .await
        .1["duplicates"],
        1
    );
    assert_eq!(
        call(
            engine.router(),
            "/v1/batch",
            &kb.token,
            batch(id, "same-user")
        )
        .await
        .1["accepted"],
        1
    );
    let raw = storage
        .connection()
        .query_one_raw(sql(
            "SELECT properties,context FROM clickman_events WHERE project_id=$1 AND message_id=$2",
            vec![a.into(), id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        raw.try_get::<Value>("", "properties").unwrap(),
        json!({"contact.email":"[FILTERED]","format":"mp4"})
    );
    assert_eq!(
        raw.try_get::<Value>("", "context").unwrap()["os.name"],
        "iOS"
    );
    engine.jobs().rotate(a, Utc::now()).await.unwrap();
    engine.jobs().rotate(b, Utc::now()).await.unwrap();
    let (status, visible) = graphql(
        &engine,
        &reader.token,
        "{projects{edges{node{id name canAdmin}}}}",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        visible["data"]["projects"]["edges"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(visible["data"]["projects"]["edges"][0]["node"]["name"], "A");
    assert_eq!(
        visible["data"]["projects"]["edges"][0]["node"]["canAdmin"],
        false
    );
    assert!(
        graphql(
            &engine,
            &reader.token,
            "query($id:UUID!){project(id:$id){name}}",
            json!({"id":b})
        )
        .await
        .1["errors"]
            .is_array()
    );
    assert!(
        graphql(
            &engine,
            &reader.token,
            "query($id:UUID!){project(id:$id){writeKeys{id}}}",
            json!({"id":a})
        )
        .await
        .1["errors"]
            .is_array()
    );
    assert!(
        graphql(
            &engine,
            &admin.token,
            "mutation($id:UUID!){issueWriteKey(projectId:$id,source:\"x\"){token}}",
            json!({"id":b})
        )
        .await
        .1["errors"]
            .is_array()
    );
    assert!(
        graphql(
            &engine,
            &admin.token,
            "mutation{createProject(slug:\"forbidden\",name:\"Forbidden\"){id}}",
            json!({})
        )
        .await
        .1["errors"]
            .is_array()
    );
    assert_eq!(
        graphql(&engine, &ka.token, "{projects{edges{node{id}}}}", json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            engine.router(),
            "/v1/batch",
            &admin.token,
            batch(Uuid::new_v4(), "x")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let reports = storage.reports(a).await.unwrap();
    let totals = &reports.iter().find(|r| r.key == "events").unwrap().result;
    assert_eq!(totals["events"][0]["events"], 1);
    assert_eq!(totals["events"][0]["actors"], 1);
    assert!(storage.erase(a, "same-user", Utc::now()).await.unwrap());
    assert!(storage.reports(a).await.unwrap().is_empty());
    storage.refresh_reports(a, Utc::now()).await.unwrap();
    assert!(
        storage
            .reports(a)
            .await
            .unwrap()
            .iter()
            .find(|r| r.key == "events")
            .unwrap()
            .result["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        storage
            .reports(b)
            .await
            .unwrap()
            .iter()
            .find(|r| r.key == "events")
            .unwrap()
            .result["events"][0]["events"],
        1
    );
    storage.revoke_write_key(a, ka.id).await.unwrap();
    assert_eq!(
        call(
            engine.router(),
            "/v1/batch",
            &ka.token,
            batch(Uuid::new_v4(), "x")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    storage.revoke_access_key(a, reader.id).await.unwrap();
    assert_eq!(
        graphql(
            &engine,
            &reader.token,
            "{projects{edges{node{id}}}}",
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    remove(&storage, &[a, b]).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn rotation_tops_up_chunks_handles_late_events_and_competing_workers() {
    let storage = storage().await;
    let p = project(&storage, "Chunks").await;
    let now = Utc::now();
    let rows: Vec<_> = (0..4097)
        .map(|i| {
            event(
                Uuid::new_v4(),
                if i % 2 == 0 { "user" } else { "*" },
                "open",
                now - Duration::minutes(10),
            )
        })
        .collect();
    assert_eq!(
        storage
            .insert_server(p, "server", &rows[..3000], now - Duration::minutes(5))
            .await
            .unwrap(),
        3000
    );
    assert_eq!(storage.rotate(p, now).await.unwrap(), 3000);
    storage
        .insert_server(p, "server", &rows[3000..], now - Duration::minutes(5))
        .await
        .unwrap();
    let (one, two) = tokio::join!(storage.rotate(p, now), storage.rotate(p, now));
    assert_eq!(one.unwrap() + two.unwrap(), 1097);
    let chunks = storage
        .connection()
        .query_all_raw(sql(
            "SELECT events FROM clickman_chunks WHERE project_id=$1 ORDER BY seq",
            vec![p.into()],
        ))
        .await
        .unwrap();
    assert_eq!(
        chunks
            .iter()
            .map(|r| r.try_get::<i32>("", "events").unwrap())
            .collect::<Vec<_>>(),
        vec![4096, 1]
    );
    // A transaction with an old receive time can become visible after a rotation.
    storage
        .insert_server(
            p,
            "server",
            &[event(
                Uuid::new_v4(),
                "late",
                "open",
                now - Duration::days(1),
            )],
            now - Duration::hours(1),
        )
        .await
        .unwrap();
    assert_eq!(storage.rotate(p, now).await.unwrap(), 1);
    storage.refresh_reports(p, now).await.unwrap();
    let reports = storage.reports(p).await.unwrap();
    let totals = &reports.iter().find(|r| r.key == "events").unwrap().result;
    assert_eq!(totals["events"][0]["events"], 4098);
    assert_eq!(totals["events"][0]["actors"], 2);
    let name = storage
        .connection()
        .query_one_raw(sql(
            "SELECT id FROM clickman_event_names WHERE project_id=$1 LIMIT 1",
            vec![p.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "id")
        .unwrap();
    let other = project(&storage, "Other").await;
    assert!(storage.connection().execute_raw(sql("INSERT INTO clickman_daily_counts(project_id,day,event_name_id,events,actors) VALUES($1,$2,$3,1,1)",vec![other.into(),now.date_naive().into(),name.into()])).await.is_err());
    remove(&storage, &[p, other]).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn failed_rotation_rolls_back_and_server_tracking_uses_the_protocol_rules() {
    let storage = storage().await;
    let p = project(&storage, "Rollback").await;
    let engine = engine(storage.clone());
    let mut tracked = TrackingEvent::new(
        "open",
        "actor",
        json!({"contact":{"email":"private@example.com"}}),
    );
    let id = tracked.message_id;
    assert!(engine.track(p, "api", tracked).await.unwrap());
    tracked = TrackingEvent::new("open", "actor", json!({}));
    tracked.message_id = id;
    assert!(!engine.track(p, "api", tracked).await.unwrap());
    storage.rotate(p, Utc::now()).await.unwrap();
    let good = storage
        .connection()
        .query_one_raw(sql(
            "SELECT payload FROM clickman_chunks WHERE project_id=$1",
            vec![p.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Vec<u8>>("", "payload")
        .unwrap();
    storage
        .connection()
        .execute_raw(sql(
            "UPDATE clickman_chunks SET payload=$2 WHERE project_id=$1",
            vec![p.into(), b"corrupt".to_vec().into()],
        ))
        .await
        .unwrap();
    assert!(
        engine
            .track(p, "api", TrackingEvent::new("open", "actor", json!({})))
            .await
            .unwrap()
    );
    assert!(storage.rotate(p, Utc::now()).await.is_err());
    let count=storage.connection().query_one_raw(sql("SELECT count(*)::bigint AS n FROM clickman_events WHERE project_id=$1 AND rotated_at IS NULL",vec![p.into()])).await.unwrap().unwrap().try_get::<i64>("","n").unwrap();
    assert_eq!(count, 1);
    storage
        .connection()
        .execute_raw(sql(
            "UPDATE clickman_chunks SET payload=$2 WHERE project_id=$1",
            vec![p.into(), good.into()],
        ))
        .await
        .unwrap();
    assert_eq!(storage.rotate(p, Utc::now()).await.unwrap(), 1);
    remove(&storage, &[p]).await;
}

struct Recording {
    fail: AtomicBool,
    calls: AtomicUsize,
}
#[async_trait]
impl Dispatcher for Recording {
    async fn deliver(
        &self,
        _: &Destination,
        events: &[StoredEvent],
    ) -> std::result::Result<(), DeliveryError> {
        assert_eq!(events.len(), 1);
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            Err(DeliveryError("injected outage".into()))
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn delivery_retry_and_raw_retention_survive_reopen() {
    let storage = storage().await;
    let p = project(&storage, "Outbox").await;
    let now = Utc::now();
    let d = storage
        .save_destination(p, "recording", "custom", json!({}))
        .await
        .unwrap();
    storage
        .insert_server(
            p,
            "api",
            &[event(
                Uuid::new_v4(),
                "actor",
                "open",
                now - Duration::days(8),
            )],
            now - Duration::days(8),
        )
        .await
        .unwrap();
    storage.rotate(p, now).await.unwrap();
    assert_eq!(storage.prune(p, now).await.unwrap(), 0);
    let dispatcher = Arc::new(Recording {
        fail: AtomicBool::new(true),
        calls: AtomicUsize::new(0),
    });
    let first = engine(storage.clone()).with_dispatcher(dispatcher.clone());
    assert_eq!(first.jobs().deliver(p, Utc::now()).await.unwrap(), 0);
    assert_eq!(dispatcher.calls.load(Ordering::SeqCst), 1);
    assert!(
        storage
            .delivery_page(p, d.id, now, 2000)
            .await
            .unwrap()
            .is_none()
    );
    let reopened = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 4)
        .await
        .unwrap();
    dispatcher.fail.store(false, Ordering::SeqCst);
    let second = engine(reopened.clone()).with_dispatcher(dispatcher.clone());
    assert_eq!(
        second
            .jobs()
            .deliver(p, now + Duration::hours(1))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        reopened.prune(p, now + Duration::hours(1)).await.unwrap(),
        1
    );
    assert_eq!(dispatcher.calls.load(Ordering::SeqCst), 2);
    remove(&storage, &[p]).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn funnels_filters_retention_and_embedding_work_with_postgres() {
    let storage = storage().await;
    let p = project(&storage, "Funnel").await;
    let now = Utc::now();
    storage.save_funnel(p,json!({"key":"export","name":"Export","steps":[{"event":"open"},{"event":"export","where":{"format":"mp4"}}],"window":"1d","breakdown":"context.os.name"})).await.unwrap();
    let rows = vec![
        event(Uuid::new_v4(), "actor", "open", now - Duration::hours(2)),
        event(Uuid::new_v4(), "actor", "export", now - Duration::hours(1)),
        event(Uuid::new_v4(), "actor", "open", now - Duration::days(1)),
        event(Uuid::new_v4(), "*", "open", now - Duration::hours(3)),
    ];
    storage
        .insert_server(p, "api", &rows, now - Duration::minutes(5))
        .await
        .unwrap();
    storage.rotate(p, now).await.unwrap();
    storage.refresh_reports(p, now).await.unwrap();
    let report = storage
        .reports(p)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.key == "funnel:export")
        .unwrap()
        .result;
    assert_eq!(report["steps"][0]["actors"], 1);
    assert_eq!(report["steps"][1]["actors"], 1);
    assert_eq!(report["breakdown"][0]["value"], "iOS");
    let engine = engine(storage.clone());
    let host = Router::new()
        .route("/host", axum::routing::get(|| async { "host" }))
        .nest("/analytics", engine.router());
    let result = call(
        host,
        "/analytics/graphql",
        ROOT,
        json!({"query":"{viewer{canCreateProjects}}"}),
    )
    .await;
    assert_eq!(result.1["data"]["viewer"]["canCreateProjects"], true);
    let old = now - Duration::days(120);
    storage
        .insert_server(
            p,
            "api",
            &[event(Uuid::new_v4(), "old", "ancient", old)],
            old,
        )
        .await
        .unwrap();
    storage.rotate(p, now).await.unwrap();
    storage
        .update_settings(
            p,
            ProjectSettings {
                retention_days: Some(30),
                rotation_lag_seconds: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    storage.prune(p, now).await.unwrap();
    let earliest = storage
        .connection()
        .query_one_raw(sql(
            "SELECT min(day) AS day FROM clickman_chunks WHERE project_id=$1",
            vec![p.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Option<chrono::NaiveDate>>("", "day")
        .unwrap()
        .unwrap();
    assert!(earliest > old.date_naive());
    storage.delete_funnel(p, "export").await.unwrap();
    assert!(
        !storage
            .reports(p)
            .await
            .unwrap()
            .iter()
            .any(|r| r.key == "funnel:export")
    );
    remove(&storage, &[p]).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; run scripts/test-postgres"]
async fn scoped_storage_and_graphql_share_the_callers_transaction() {
    let storage = storage().await;
    let engine = engine(storage.clone());
    let tx = Arc::new(storage.connection().begin().await.unwrap());
    let scoped = Storage::from_transaction(tx.clone()).unwrap();
    assert!(matches!(
        scoped.clone().close().await,
        Err(clickman_storage::Error::Invalid(_))
    ));
    assert!(
        scoped.healthy().await,
        "close must not finish the caller's transaction"
    );

    let slug = format!("scoped-{}", Uuid::new_v4().simple());
    let response = engine
        .schema()
        .execute(
            async_graphql::Request::new(format!(
                "mutation {{ createProject(slug: \"{slug}\", name: \"Scoped\") {{ id }} }}"
            ))
            .data(clickman_engine::Access::root())
            .data(scoped.clone()),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let id: Uuid = data["createProject"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let settings = ProjectSettings {
        rotation_lag_seconds: 17,
        ..Default::default()
    };
    // This method begins and commits an internal transaction. In the scoped
    // view that is a savepoint, so even its completed work remains private.
    scoped.update_settings(id, settings).await.unwrap();
    let key = scoped.issue_write_key(id, "ios").await.unwrap();
    assert_eq!(scoped.project(id).await.unwrap().slug, slug);
    assert!(storage.projects().await.unwrap().iter().all(|p| p.id != id));
    assert!(storage.write_identity(&key.token).await.unwrap().is_none());
    drop(scoped);
    Arc::try_unwrap(tx)
        .expect("completed GraphQL requests must release their scoped storage")
        .rollback()
        .await
        .unwrap();
    assert!(storage.projects().await.unwrap().iter().all(|p| p.id != id));
    assert!(storage.write_identity(&key.token).await.unwrap().is_none());

    let tx = Arc::new(storage.connection().begin().await.unwrap());
    let scoped = Storage::from_transaction(tx.clone()).unwrap();
    let committed = scoped
        .create_project(&format!("scoped-{}", Uuid::new_v4().simple()), "Committed")
        .await
        .unwrap();
    scoped
        .update_settings(
            committed.id,
            ProjectSettings {
                rotation_lag_seconds: 23,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    drop(scoped);
    Arc::try_unwrap(tx).unwrap().commit().await.unwrap();
    let committed = storage.project(committed.id).await.unwrap();
    let settings: ProjectSettings = serde_json::from_value(committed.settings.clone()).unwrap();
    assert_eq!(settings.rotation_lag_seconds, 23);
    remove(&storage, &[committed.id]).await;
}
