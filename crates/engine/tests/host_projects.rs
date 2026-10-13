use clickman_graphql::ProjectCreator;
use clickman_storage::{Access, Project, Result, Storage};
use sea_orm::TransactionTrait;
use std::sync::Arc;
use uuid::Uuid;

struct ScopedCreator {
    namespace: String,
}
#[async_trait::async_trait]
impl ProjectCreator for ScopedCreator {
    async fn create(&self, storage: &Storage, slug: &str, name: &str) -> Result<Project> {
        storage
            .create_project_in_namespace(&self.namespace, slug, name)
            .await
    }
}

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn host_creation_is_scoped_preserves_reader_permissions_and_rolls_back_with_the_host() {
    let storage = Storage::connect(
        &std::env::var("TEST_DATABASE_URL").expect("use scripts/test-platform"),
        8,
    )
    .await
    .unwrap();
    storage.migrate().await.unwrap();
    let slug = format!("host-{}", Uuid::new_v4().simple());
    let other = storage
        .create_project_in_namespace("other-org", &slug, "Other")
        .await
        .unwrap();
    assert!(
        storage
            .create_project_in_namespace("other-org", &slug, "Duplicate")
            .await
            .is_err()
    );
    let tx = Arc::new(storage.connection().begin().await.unwrap());
    let scoped = storage.in_transaction(tx.clone()).unwrap();
    let creator: Arc<dyn ProjectCreator> = Arc::new(ScopedCreator {
        namespace: "authorized-org".into(),
    });
    let schema = clickman_graphql::schema(scoped.clone());
    let unauthenticated = schema
        .execute(
            async_graphql::Request::new(
                "mutation{createProject(slug:\"missing-access\",name:\"Missing\"){id}}",
            )
            .data(creator.clone()),
        )
        .await;
    assert!(
        !unauthenticated.errors.is_empty(),
        "a host capability still requires authenticated request access"
    );
    let response = schema
        .execute(
            async_graphql::Request::new(format!(
                "mutation{{createProject(slug:\"{slug}\",name:\"Authorized\"){{id slug}}}}"
            ))
            .data(Access::projects(Default::default()))
            .data(creator.clone()),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let created = Uuid::parse_str(data["createProject"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        scoped.project(created).await.unwrap().namespace,
        "authorized-org"
    );
    assert!(storage.project(created).await.is_err());
    let reader = schema
        .execute(
            async_graphql::Request::new(format!("query{{project(id:\"{}\"){{id}}}}", other.id))
                .data(Access::projects(Default::default()))
                .data(creator.clone()),
        )
        .await;
    assert!(
        !reader.errors.is_empty(),
        "creation capability must not grant global read access"
    );
    drop(schema);
    drop(scoped);
    drop(creator);
    Arc::try_unwrap(tx)
        .unwrap_or_else(|_| panic!("creation retained the host transaction"))
        .rollback()
        .await
        .unwrap();
    assert!(storage.project(created).await.is_err());
    let denied = clickman_graphql::schema(storage)
        .execute(
            async_graphql::Request::new(
                "mutation{createProject(slug:\"forbidden\",name:\"Forbidden\"){id}}",
            )
            .data(Access::projects(Default::default())),
        )
        .await;
    assert!(!denied.errors.is_empty());
}
