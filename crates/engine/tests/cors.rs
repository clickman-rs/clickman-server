use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use clickman_engine::Engine;
use clickman_storage::{ProjectSettings, Storage};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

async fn post(
    router: Router,
    path: &str,
    origin: Option<&str>,
    key: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut request = Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {key}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    router
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires owned PostgreSQL"]
async fn browser_origin_policy_applies_to_batch_and_identity_without_exposing_graphql() {
    assert_eq!(std::env::var("CLICKMAN_TEST_OWNED").unwrap(), "1");
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 8)
        .await
        .unwrap();
    storage.migrate().await.unwrap();
    let project = storage
        .create_project(&format!("cors-{}", Uuid::new_v4()), "Browser origin")
        .await
        .unwrap();
    storage
        .update_settings(
            project.id,
            ProjectSettings {
                allowed_origins: vec!["https://site.example".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let source = storage.issue_write_key(project.id, "web").await.unwrap();
    let engine = Engine::new(storage.clone()).unwrap();
    let batch = || {
        let now = chrono::Utc::now().to_rfc3339();
        json!({"sentAt":now,"batch":[{"type":"track","messageId":Uuid::new_v4(),"timestamp":now,"externalId":"browser-user","event":"view"}]})
    };
    let response = post(
        engine.router(),
        "/v1/batch",
        Some("https://site.example"),
        &source.token,
        batch(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(
        response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert_eq!(
        post(
            engine.router(),
            "/v1/batch",
            Some("https://other.example"),
            &source.token,
            batch()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(engine.router(), "/v1/batch", None, &source.token, batch())
            .await
            .status(),
        StatusCode::ACCEPTED,
        "native clients have no browser Origin"
    );
    let link = json!({"anonymousId":format!("anon:{}",Uuid::new_v4()),"externalId":"browser-user"});
    assert_eq!(
        post(
            engine.router(),
            "/v1/identify",
            Some("https://other.example"),
            &source.token,
            link.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = post(
        engine.router(),
        "/v1/identify",
        Some("https://site.example"),
        &source.token,
        link,
    )
    .await;
    assert!(response.status().is_success());
    assert!(
        response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    let preflight = engine
        .router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/identify")
                .header(header::ORIGIN, "https://site.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "authorization,content-type",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(preflight.status().is_success());
    assert!(
        preflight
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    let graph = post(
        engine.router(),
        "/graphql",
        Some("https://site.example"),
        &source.token,
        json!({"query":"{projects{edges{node{id}}}}"}),
    )
    .await;
    assert_eq!(graph.status(), StatusCode::UNAUTHORIZED);
    assert!(
        !graph
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    storage
        .revoke_write_key(project.id, source.id)
        .await
        .unwrap();
    assert_eq!(
        post(
            engine.router(),
            "/v1/identify",
            Some("https://site.example"),
            &source.token,
            json!({"anonymousId":format!("anon:{}",Uuid::new_v4()),"externalId":"browser-user"})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
