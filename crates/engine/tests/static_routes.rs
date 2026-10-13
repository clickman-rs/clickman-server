use axum::{body::Body, http::Request};
use clickman_engine::Engine;
use clickman_storage::Storage;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn dashboard_direct_links_return_success_without_masking_api_authentication() {
    assert_eq!(std::env::var("CLICKMAN_TEST_OWNED").as_deref(), Ok("1"));
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 2)
        .await
        .unwrap();
    let engine = Engine::new(storage).unwrap();
    let assets = std::env::temp_dir().join(format!("clickman-static-{}", Uuid::new_v4()));
    tokio::fs::create_dir(&assets).await.unwrap();
    tokio::fs::write(
        assets.join("index.html"),
        "<html>owned dashboard fixture</html>",
    )
    .await
    .unwrap();
    let router = engine.router_with_dashboard(&assets);
    for path in [
        "/".to_owned(),
        format!("/projects/{}/retention", Uuid::new_v4()),
    ] {
        let response = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"<html>owned dashboard fixture</html>");
    }
    let response = router
        .oneshot(
            Request::post("/graphql")
                .header("content-type", "application/json")
                .body(Body::from("{\"query\":\"{projects{totalCount}}\"}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    tokio::fs::remove_dir_all(assets).await.unwrap();
}
