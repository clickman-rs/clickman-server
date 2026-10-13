use clickman_graphql::ProjectCreator;
use clickman_storage::{Access, Project, Result, Role, Storage};
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

struct AdminCreator(ScopedCreator);
#[async_trait::async_trait]
impl ProjectCreator for AdminCreator {
    fn created_role(&self) -> Role {
        Role::Admin
    }
    async fn create(&self, storage: &Storage, slug: &str, name: &str) -> Result<Project> {
        self.0.create(storage, slug, name).await
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
                "mutation{{createProject(slug:\"{slug}\",name:\"Authorized\"){{id slug canAdmin reports{{key}} funnels{{key}}}}}}"
            ))
            .data(Access::projects(Default::default()))
            .data(creator.clone()),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["createProject"]["canAdmin"], false);
    assert_eq!(data["createProject"]["reports"], serde_json::json!([]));
    let created = Uuid::parse_str(data["createProject"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        scoped.project(created).await.unwrap().namespace,
        "authorized-org"
    );
    assert!(storage.project(created).await.is_err());
    let private = schema.execute(async_graphql::Request::new(format!(
        "mutation{{createProject(slug:\"reader-{}\",name:\"Reader\"){{id writeKeys{{id}}}}}}", Uuid::new_v4().simple()
    )).data(Access::default()).data(creator.clone())).await;
    assert_eq!(
        private.errors[0].message, "project administration denied",
        "default created role must not expose admin fields"
    );
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

#[tokio::test]
#[ignore = "requires owned PostgreSQL; use scripts/test-platform"]
async fn created_admin_node_can_select_nested_fields_without_broadening_request_access() {
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 4)
        .await
        .unwrap();
    storage.migrate().await.unwrap();
    let other = storage
        .create_project(&format!("other-{}", Uuid::new_v4().simple()), "Other")
        .await
        .unwrap();
    let tx = Arc::new(storage.connection().begin().await.unwrap());
    let scoped = storage.in_transaction(tx.clone()).unwrap();
    let creator: Arc<dyn ProjectCreator> = Arc::new(AdminCreator(ScopedCreator {
        namespace: "creator-admin".into(),
    }));
    let schema = clickman_graphql::schema(scoped.clone());
    let slug = format!("created-{}", Uuid::new_v4().simple());
    let response = schema.execute(async_graphql::Request::new(format!(
        "mutation{{createProject(slug:\"{slug}\",name:\"Created\"){{id canAdmin reports{{key}} funnels{{key}} writeKeys{{id}}}}}}"
    )).data(Access::default()).data(creator.clone())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["createProject"]["canAdmin"], true);
    for field in ["reports", "funnels", "writeKeys"] {
        assert_eq!(data["createProject"][field], serde_json::json!([]));
    }
    let created = Uuid::parse_str(data["createProject"]["id"].as_str().unwrap()).unwrap();
    assert!(storage.project(created).await.is_err());
    let next = schema
        .execute(
            async_graphql::Request::new(format!("query{{project(id:\"{created}\"){{id}}}}"))
                .data(Access::default())
                .data(creator.clone()),
        )
        .await;
    assert!(
        !next.errors.is_empty(),
        "returned-node grant leaked to the next host request"
    );
    let next = schema.execute(async_graphql::Request::new(format!("mutation{{saveFunnel(projectId:\"{created}\",definition:{{key:\"buy\",name:\"Buy\",steps:[{{event:\"view\"}},{{event:\"buy\"}}]}}){{key}}}}"))
        .data(Access::default()).data(creator.clone())).await;
    assert!(
        !next.errors.is_empty(),
        "returned-node grant leaked into mutation authorization"
    );
    let second_slug = format!("rollback-{}", Uuid::new_v4().simple());
    let response = schema.execute(async_graphql::Request::new(format!(
        "mutation{{createProject(slug:\"{second_slug}\",name:\"Rollback\"){{id canAdmin reports{{key}}}} denied:saveFunnel(projectId:\"{}\",definition:{{key:\"buy\",name:\"Buy\",steps:[{{event:\"view\"}},{{event:\"buy\"}}]}}){{key}}}}", other.id
    )).data(Access::default()).data(creator.clone())).await;
    assert!(
        !response.errors.is_empty(),
        "returned-node grant authorized an unrelated project in the same request"
    );
    let second = scoped
        .projects()
        .await
        .unwrap()
        .into_iter()
        .find(|project| project.slug == second_slug)
        .unwrap()
        .id;
    assert!(scoped.funnels(other.id).await.unwrap().is_empty());
    drop(schema);
    drop(scoped);
    drop(creator);
    Arc::try_unwrap(tx)
        .unwrap_or_else(|_| panic!("created-node authorization retained the transaction"))
        .rollback()
        .await
        .unwrap();
    assert!(storage.project(created).await.is_err());
    assert!(storage.project(second).await.is_err());
}
