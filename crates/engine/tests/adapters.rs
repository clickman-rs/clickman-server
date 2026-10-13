use chrono::{DateTime, Duration, Utc};
use clickhouse::Client;
use clickman_protocol::Event;
use clickman_storage::{
    ProjectSettings, Storage,
    adapters::{ClickHouseAdapter, ClickHouseConfig},
    sql,
};
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

async fn postgres() -> Storage {
    let storage = Storage::connect(
        &std::env::var("TEST_DATABASE_URL").expect("use scripts/test-platform"),
        8,
    )
    .await
    .unwrap();
    storage.migrate().await.unwrap();
    storage
}

async fn clickhouse(base: &Storage) -> (Storage, Client) {
    let url = std::env::var("TEST_CLICKHOUSE_URL").expect("use scripts/test-platform --clickhouse");
    let user = std::env::var("TEST_CLICKHOUSE_USER").unwrap();
    let password = std::env::var("TEST_CLICKHOUSE_PASSWORD").unwrap();
    let database = format!("contract_{}", Uuid::new_v4().simple());
    let client = Client::default()
        .with_url(&url)
        .with_user(&user)
        .with_password(&password)
        .with_database(&database);
    let backend = ClickHouseAdapter::new(ClickHouseConfig {
        url,
        user,
        password,
        database,
    })
    .unwrap();
    let storage = base.clone().with_analytics_adapter(Arc::new(backend));
    storage.migrate().await.unwrap();
    assert!(storage.healthy().await);
    (storage, client)
}

async fn project(storage: &Storage) -> Uuid {
    let id = storage
        .create_project(
            &format!("contract-{}", Uuid::new_v4().simple()),
            "Backend contract",
        )
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
    storage.save_funnel(id, json!({"key":"purchase","name":"Purchase","window":"7d","range":"90d","steps":[{"event":"view"},{"event":"purchase","where":{"format":"mp4"}}]})).await.unwrap();
    id
}

fn event(id: Uuid, actor: &str, name: &str, format: &str, at: DateTime<Utc>) -> Event {
    Event {
        message_id: id,
        external_id: actor.into(),
        event: name.into(),
        occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
            at.timestamp_nanos_opt().unwrap().into(),
        )
        .unwrap(),
        properties: json!({"format":format}).as_object().unwrap().clone(),
        context: json!({"os.name":"web"}).as_object().unwrap().clone(),
    }
}
async fn report(storage: &Storage, project: Uuid, now: DateTime<Utc>) -> BTreeMap<String, Value> {
    storage.rotate(project, now).await.unwrap();
    storage.refresh_reports(project, now).await.unwrap();
    storage
        .reports(project)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.key, r.result))
        .collect()
}
async fn count(client: &Client, project: Uuid) -> u64 {
    client
        .query("SELECT count() FROM events FINAL WHERE project_id=?")
        .bind(project)
        .fetch_one::<u64>()
        .await
        .unwrap()
}
async fn segment_contract(storage: &Storage, id: Uuid) {
    let now = Utc::now();
    let s=storage.save_segment(id,json!({"key":"inactive","name":"Inactive","rule":{"kind":"inactive_after","entry":{"event":"view","where":{"format":"mp4"}},"absent":{"event":"purchase"},"after":"24h","range":"7d"}}),true,false).await.unwrap();
    let hook=storage.save_webhook(id,s.id,"Catch Hook","zapier",json!({"url":"https://hooks.zapier.com/hooks/catch/fixture/fixture/","signing_ref":format!("{}SIGNING",clickman_destinations::credential_prefix(id))}),true).await.unwrap();
    storage
        .insert_server(
            id,
            "web",
            &[
                event(
                    Uuid::new_v4(),
                    "history",
                    "view",
                    "mp4",
                    now - Duration::hours(48),
                ),
                event(
                    Uuid::new_v4(),
                    "new",
                    "view",
                    "mp4",
                    now + Duration::seconds(1),
                ),
                event(
                    Uuid::new_v4(),
                    "buyer",
                    "view",
                    "mp4",
                    now + Duration::seconds(1),
                ),
                event(
                    Uuid::new_v4(),
                    "buyer",
                    "purchase",
                    "mp4",
                    now + Duration::hours(2),
                ),
                event(Uuid::new_v4(), "*", "view", "mp4", now),
            ],
            now,
        )
        .await
        .unwrap();
    storage.rotate(id, now + Duration::hours(23)).await.unwrap();
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::hours(23))
            .await
            .unwrap(),
        0
    );
    assert!(storage.webhook_logs(id, 100).await.unwrap().is_empty());
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::hours(25))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::hours(25))
            .await
            .unwrap(),
        0
    );
    let logs = storage.webhook_logs(id, 100).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].webhook_id, hook.id);
    assert_eq!(storage.segments(id).await.unwrap()[0].members, 2);
    // Timers/ranges update membership even when no additional batch arrives.
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::days(8))
            .await
            .unwrap(),
        0
    );
    assert_eq!(storage.segments(id).await.unwrap()[0].members, 0);
    storage
        .insert_server(
            id,
            "web",
            &[event(
                Uuid::new_v4(),
                "new",
                "view",
                "mp4",
                now + Duration::days(8),
            )],
            now + Duration::days(8),
        )
        .await
        .unwrap();
    storage
        .rotate(id, now + Duration::days(9) + Duration::seconds(2))
        .await
        .unwrap();
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::days(9) + Duration::seconds(2))
            .await
            .unwrap(),
        1
    );
    assert_eq!(storage.webhook_logs(id, 100).await.unwrap().len(), 2);
    let referenced=storage.save_segment(id,json!({"key":"funnel_stage","name":"Funnel stage","rule":{"kind":"funnel","funnel_key":"purchase","at_step":2}}),true,false).await.unwrap();
    assert!(storage.delete_funnel(id, "purchase").await.is_err());
    storage.delete_segment(id, referenced.id).await.unwrap();
    assert!(storage.delete_funnel(id, "purchase").await.unwrap());
    // Identification inherits membership instead of producing a duplicate.
    let visitor = format!("anon:{}", Uuid::new_v4());
    let issued = storage.issue_write_key(id, "web").await.unwrap();
    let identity = storage
        .write_identity(&issued.token)
        .await
        .unwrap()
        .unwrap();
    storage
        .insert(
            &identity,
            &[event(
                Uuid::new_v4(),
                &visitor,
                "view",
                "mp4",
                now + Duration::days(9),
            )],
            now + Duration::days(9),
        )
        .await
        .unwrap();
    storage.rotate(id, now + Duration::days(11)).await.unwrap();
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::days(11))
            .await
            .unwrap(),
        1
    );
    storage
        .link_identity(
            &identity,
            &visitor,
            "linked-account",
            now + Duration::days(11),
        )
        .await
        .unwrap();
    assert_eq!(
        storage
            .evaluate_segments(id, now + Duration::days(11))
            .await
            .unwrap(),
        0
    );
    let before = storage.webhook_logs(id, 100).await.unwrap().len();
    assert!(
        storage
            .erase(id, "linked-account", now + Duration::days(11))
            .await
            .unwrap()
    );
    assert_eq!(
        storage.webhook_logs(id, 100).await.unwrap().len(),
        before - 1
    );
    let last = storage.webhook_logs(id, 100).await.unwrap()[0].id;
    assert!(storage.replay_webhook(id, last).await.unwrap());
    storage
        .save_webhook(id, s.id, "Catch Hook", "zapier", hook.config, false)
        .await
        .unwrap();
    assert!(!storage.replay_webhook(id, last).await.unwrap());
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL"]
async fn postgres_segments_timers_backfill_transitions_identity_and_erasure() {
    let pg = postgres().await;
    let id = project(&pg).await;
    segment_contract(&pg, id).await;
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse"]
async fn clickhouse_segments_match_shared_transition_and_timer_contract() {
    let pg = postgres().await;
    let (ch, _) = clickhouse(&pg).await;
    let id = project(&ch).await;
    segment_contract(&ch, id).await;
}

async fn identity_contract(storage: &Storage, project: Uuid) -> BTreeMap<String, Value> {
    let now = Utc::now();
    let visitor = format!("anon:{}", Uuid::new_v4());
    let issued = storage.issue_write_key(project, "web").await.unwrap();
    let identity = storage
        .write_identity(&issued.token)
        .await
        .unwrap()
        .unwrap();
    storage
        .insert(
            &identity,
            &[
                event(
                    Uuid::new_v4(),
                    &visitor,
                    "view",
                    "mp4",
                    now - Duration::days(8),
                ),
                event(
                    Uuid::new_v4(),
                    "account",
                    "purchase",
                    "mp4",
                    now - Duration::days(7),
                ),
                event(Uuid::new_v4(), "*", "view", "mp4", now),
            ],
            now,
        )
        .await
        .unwrap();
    let before = report(storage, project, now).await;
    assert_eq!(before["funnel:purchase"]["steps"][1]["actors"], 0);
    storage
        .link_identity(&identity, &visitor, "account", now)
        .await
        .unwrap();
    storage
        .link_identity(&identity, &visitor, "account", now)
        .await
        .unwrap();
    assert!(
        storage
            .link_identity(&identity, &visitor, "another-account", now)
            .await
            .is_err()
    );
    let after = report(storage, project, now).await;
    assert_eq!(after["funnel:purchase"]["steps"][1]["actors"], 1);
    storage
        .insert(
            &identity,
            &[event(
                Uuid::new_v4(),
                &visitor,
                "view",
                "mp4",
                now - Duration::days(9),
            )],
            now,
        )
        .await
        .unwrap();
    let linked = report(storage, project, now).await;
    assert_eq!(linked["funnel:purchase"]["steps"][0]["actors"], 1);
    assert_eq!(linked["funnel:purchase"]["steps"][1]["actors"], 1);
    assert_eq!(linked["events"]["events"][0]["actors"], 1);
    let cohort:chrono::NaiveDate=storage.connection().query_one_raw(sql("SELECT first_seen_on FROM clickman_actors WHERE project_id=$1 AND external_id='account'",vec![project.into()])).await.unwrap().unwrap().try_get("","first_seen_on").unwrap();
    assert_eq!(cohort, (now - Duration::days(9)).date_naive());
    assert!(storage.erase(project, "account", now).await.unwrap());
    let erased = report(storage, project, now).await;
    assert_eq!(erased["events"]["events"][0]["events"], 1);
    assert_eq!(erased["events"]["events"][0]["actors"], 0);
    assert_eq!(erased["funnel:purchase"]["steps"][0]["actors"], 0);
    assert!(
        storage
            .write_identity(&issued.token)
            .await
            .unwrap()
            .is_some()
    );
    storage.revoke_write_key(project, issued.id).await.unwrap();
    assert!(
        storage
            .link_identity(&identity, &visitor, "account", now)
            .await
            .is_err()
    );
    linked
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL"]
async fn postgres_identity_links_fold_reports_cohorts_and_erase_the_whole_family() {
    let pg = postgres().await;
    let id = project(&pg).await;
    identity_contract(&pg, id).await;
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse"]
async fn clickhouse_identity_links_match_postgres_and_erase_native_family() {
    let pg = postgres().await;
    let (ch, native) = clickhouse(&pg).await;
    let id = project(&ch).await;
    let result = identity_contract(&ch, id).await;
    let pg_id = project(&pg).await;
    assert_eq!(result, identity_contract(&pg, pg_id).await);
    assert_eq!(count(&native, id).await, 1);
}

async fn identify_without_events_contract(storage: &Storage) {
    let id = project(storage).await;
    let now = Utc::now();
    let key = storage.issue_write_key(id, "web").await.unwrap();
    let identity = storage.write_identity(&key.token).await.unwrap().unwrap();
    let anonymous = format!("anon:{}", Uuid::new_v4());
    storage
        .link_identity(&identity, &anonymous, "empty-account", now)
        .await
        .unwrap();
    let reports = report(storage, id, now).await;
    assert_eq!(
        reports["retention"]["cohorts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["size"].as_u64().unwrap())
            .sum::<u64>(),
        0,
        "identify is not an activity event"
    );
    // A link created before any activity must not determine cohort age.
    let later = now + Duration::days(2);
    storage
        .insert_server(
            id,
            "web",
            &[event(Uuid::new_v4(), &anonymous, "view", "mp4", later)],
            later,
        )
        .await
        .unwrap();
    let reports = report(storage, id, later).await;
    let cohorts = reports["retention"]["cohorts"].as_array().unwrap();
    assert_eq!(
        cohorts
            .iter()
            .map(|c| c["size"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    assert_eq!(
        cohorts.iter().find(|c| c["size"] == 1).unwrap()["day"],
        later.date_naive().to_string()
    );
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL"]
async fn postgres_identify_without_events_does_not_create_cohorts() {
    identify_without_events_contract(&postgres().await).await;
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse"]
async fn clickhouse_identify_without_events_does_not_create_cohorts() {
    let pg = postgres().await;
    let (ch, _) = clickhouse(&pg).await;
    identify_without_events_contract(&ch).await;
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse; use scripts/test-platform --clickhouse"]
async fn clickhouse_matches_postgres_for_reports_replay_late_commits_retention_and_erasure() {
    let pg = postgres().await;
    let (ch, native) = clickhouse(&pg).await;
    let pg_project = project(&pg).await;
    let ch_project = project(&ch).await;
    let now = Utc::now();
    let first = Uuid::new_v4();
    let events = vec![
        event(first, "actor-a", "view", "mp4", now - Duration::days(8)),
        event(
            Uuid::new_v4(),
            "actor-a",
            "purchase",
            "mp4",
            now - Duration::days(7),
        ),
        event(
            Uuid::new_v4(),
            "actor-a",
            "view",
            "mp4",
            now - Duration::days(1),
        ),
        event(
            Uuid::new_v4(),
            "actor-b",
            "view",
            "png",
            now - Duration::hours(1),
        ),
        event(Uuid::new_v4(), "actor-b", "purchase", "png", now),
        event(Uuid::new_v4(), "*", "view", "mp4", now),
    ];
    for (store, id) in [(&pg, pg_project), (&ch, ch_project)] {
        assert_eq!(
            store.insert_server(id, "web", &events, now).await.unwrap(),
            6
        );
        assert_eq!(
            store.insert_server(id, "web", &events, now).await.unwrap(),
            0
        );
    }
    assert_eq!(
        report(&pg, pg_project, now).await,
        report(&ch, ch_project, now).await
    );
    assert_eq!(count(&native, ch_project).await, 6);
    let chunks: i64 = ch
        .connection()
        .query_one_raw(sql(
            "SELECT count(*)::bigint AS n FROM clickman_chunks WHERE project_id=$1",
            vec![ch_project.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(
        chunks, 0,
        "ClickHouse stores native events, not PostgreSQL chunks"
    );
    // Simulate losing the metadata acknowledgement after a successful remote insert.
    ch.connection()
        .execute_raw(sql(
            "UPDATE clickman_events SET rotated_at=NULL WHERE project_id=$1",
            vec![ch_project.into()],
        ))
        .await
        .unwrap();
    assert_eq!(ch.rotate(ch_project, now).await.unwrap(), 6);
    assert_eq!(count(&native, ch_project).await, 6);
    assert_eq!(
        report(&pg, pg_project, now).await,
        report(&ch, ch_project, now).await
    );
    let late = [event(
        Uuid::new_v4(),
        "actor-a",
        "view",
        "mp4",
        now - Duration::days(10),
    )];
    for (store, id) in [(&pg, pg_project), (&ch, ch_project)] {
        assert_eq!(store.insert_server(id, "web", &late, now).await.unwrap(), 1);
    }
    assert_eq!(
        report(&pg, pg_project, now).await,
        report(&ch, ch_project, now).await
    );
    // A new accepted event after the documented dedup window remains a new event.
    let later = now + Duration::days(8);
    for (store, id) in [(&pg, pg_project), (&ch, ch_project)] {
        assert_eq!(store.prune(id, later).await.unwrap(), 7);
        assert_eq!(
            store
                .insert_server(id, "web", &events[..1], later)
                .await
                .unwrap(),
            1
        );
    }
    assert_eq!(
        report(&pg, pg_project, later).await,
        report(&ch, ch_project, later).await
    );
    assert_eq!(count(&native, ch_project).await, 8);
    for (store, id) in [(&pg, pg_project), (&ch, ch_project)] {
        let issued = store.issue_write_key(id, "revocation").await.unwrap();
        let identity = store.write_identity(&issued.token).await.unwrap().unwrap();
        assert!(store.revoke_write_key(id, issued.id).await.unwrap());
        assert!(store.insert(&identity, &late, later).await.is_err());
        assert!(store.erase(id, "actor-a", later).await.unwrap());
    }
    assert_eq!(
        report(&pg, pg_project, later).await,
        report(&ch, ch_project, later).await
    );
    assert_eq!(count(&native, ch_project).await, 3);
    for (store, id) in [(&pg, pg_project), (&ch, ch_project)] {
        store
            .update_settings(
                id,
                ProjectSettings {
                    rotation_lag_seconds: 0,
                    retention_days: Some(1),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store.prune(id, later + Duration::days(65)).await.unwrap();
    }
    assert_eq!(
        report(&pg, pg_project, later + Duration::days(65)).await,
        report(&ch, ch_project, later + Duration::days(65)).await
    );
    assert_eq!(count(&native, ch_project).await, 0);
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse; use scripts/test-platform --clickhouse"]
async fn clickhouse_failure_retains_accepted_events_and_reopen_recovers_them() {
    let pg = postgres().await;
    let (good, native) = clickhouse(&pg).await;
    let id = project(&good).await;
    let bad = ClickHouseAdapter::new(ClickHouseConfig {
        url: "http://127.0.0.1:1".into(),
        database: "unused".into(),
        user: "test".into(),
        password: String::new(),
    })
    .unwrap();
    let broken = pg.clone().with_analytics_adapter(Arc::new(bad));
    let now = Utc::now();
    let events = [event(Uuid::new_v4(), "recover", "view", "mp4", now)];
    assert_eq!(
        broken.insert_server(id, "web", &events, now).await.unwrap(),
        1
    );
    assert!(broken.rotate(id, now).await.is_err());
    let pending: i64 = pg.connection().query_one_raw(sql("SELECT count(*)::bigint AS n FROM clickman_events WHERE project_id=$1 AND rotated_at IS NULL",vec![id.into()])).await.unwrap().unwrap().try_get("","n").unwrap();
    assert_eq!(pending, 1);
    assert_eq!(good.rotate(id, now).await.unwrap(), 1);
    assert_eq!(count(&native, id).await, 1);
    let tx = Arc::new(pg.connection().begin().await.unwrap());
    let scoped = good.in_transaction(tx.clone()).unwrap();
    assert_eq!(scoped.backend_name(), "clickhouse");
    assert!(
        scoped
            .rotate(id, now)
            .await
            .unwrap_err()
            .to_string()
            .contains("caller-transaction")
    );
    drop(scoped);
    Arc::try_unwrap(tx)
        .unwrap_or_else(|_| panic!("adapter retained the caller transaction"))
        .rollback()
        .await
        .unwrap();
    assert_eq!(count(&native, id).await, 1);
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse; use scripts/test-platform --clickhouse"]
async fn clickhouse_report_refresh_uses_its_held_metadata_transaction_with_one_connection() {
    let pg = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 1)
        .await
        .unwrap();
    let (storage, _) = clickhouse(&pg).await;
    let id = project(&storage).await;
    let now = Utc::now();
    storage
        .insert_server(
            id,
            "web",
            &[event(Uuid::new_v4(), "actor", "view", "mp4", now)],
            now,
        )
        .await
        .unwrap();
    storage.rotate(id, now).await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        storage.refresh_reports(id, now),
    )
    .await
    .expect("report refresh tried to acquire a second connection while holding the only connection")
    .unwrap();
    assert_eq!(storage.reports(id).await.unwrap().len(), 4);
    storage.close().await.unwrap();
}

async fn tied_predicate_projection_contract(storage: &Storage) {
    let id = project(storage).await;
    let now = Utc::now();
    let at = now - Duration::hours(1);
    storage.save_funnel(id, json!({"key":"repeat","name":"Repeat view","steps":[{"event":"view"},{"event":"view","where":{"format":"z"}}]})).await.unwrap();
    storage.save_segment(id, json!({"key":"completed","name":"Completed","rule":{"kind":"funnel","funnel_key":"repeat","at_step":2}}), true, false).await.unwrap();
    let mut first = event(Uuid::new_v4(), "same-actor", "view", "z", at);
    first.properties.insert("a_unused".into(), json!("a"));
    let mut second = event(Uuid::new_v4(), "same-actor", "view", "a", at);
    second.properties.insert("a_unused".into(), json!("z"));
    storage
        .insert_server(id, "browser", &[first, second], now)
        .await
        .unwrap();
    storage.rotate(id, now).await.unwrap();
    storage.refresh_reports(id, now).await.unwrap();
    storage.evaluate_segments(id, now).await.unwrap();
    let report = storage
        .reports(id)
        .await
        .unwrap()
        .into_iter()
        .find(|report| report.key == "funnel:repeat")
        .unwrap();
    assert_eq!(report.result["steps"][0]["actors"], 1);
    assert_eq!(report.result["steps"][1]["actors"], 1);
    let segment = storage
        .segments(id)
        .await
        .unwrap()
        .into_iter()
        .find(|segment| segment.definition.key == "completed")
        .unwrap();
    assert_eq!(segment.members, 1);
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn postgres_tied_funnel_report_and_projected_segment_observations_agree() {
    tied_predicate_projection_contract(&postgres().await).await;
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL and ClickHouse; use scripts/test-platform --clickhouse"]
async fn clickhouse_tied_funnel_report_and_projected_segment_observations_agree() {
    let pg = postgres().await;
    let (storage, _) = clickhouse(&pg).await;
    tied_predicate_projection_contract(&storage).await;
}
