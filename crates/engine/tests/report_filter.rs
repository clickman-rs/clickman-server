use async_graphql::{Request, Variables};
use chrono::Utc;
use clickman_engine::{Access, Engine, Role};
use clickman_storage::{Storage, sql};
use sea_orm::ConnectionTrait;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an owned TEST_DATABASE_URL; run scripts/test-platform"]
async fn selected_reports_are_bounded_and_scoped_before_fetching() {
    let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap(), 4)
        .await
        .unwrap();
    storage.migrate().await.unwrap();
    let a = storage
        .create_project(&format!("reports-{}", Uuid::new_v4().simple()), "Reports")
        .await
        .unwrap();
    let b = storage
        .create_project(
            &format!("reports-{}", Uuid::new_v4().simple()),
            "Other project",
        )
        .await
        .unwrap();
    for project in [a.id, b.id] {
        for key in ["events", "actives", "funnel:buy"] {
            storage.connection().execute_raw(sql(
                "INSERT INTO clickman_reports(project_id,key,result,computed_at) VALUES($1,$2,$3,$4)",
                vec![project.into(), key.into(), json!({"project":project,"key":key}).into(), Utc::now().into()],
            )).await.unwrap();
        }
    }
    let schema = Engine::new(storage.clone()).unwrap().schema().clone();
    let query =
        "query($id:UUID!,$keys:[String!]){project(id:$id){reports(keys:$keys){key result}}}";
    let execute = |id, keys| {
        let schema = schema.clone();
        async move {
            schema
                .execute(
                    Request::new(query)
                        .variables(Variables::from_json(json!({"id":id,"keys":keys})))
                        .data(Access::project(a.id, Role::Reader)),
                )
                .await
        }
    };
    let response = execute(a.id, json!(["events", "not-there"])).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().unwrap();
    assert_eq!(value["project"]["reports"].as_array().unwrap().len(), 1);
    assert_eq!(value["project"]["reports"][0]["key"], "events");
    assert_eq!(
        value["project"]["reports"][0]["result"]["project"],
        a.id.to_string()
    );
    assert_eq!(
        execute(a.id, json!([])).await.data.into_json().unwrap()["project"]["reports"],
        json!([])
    );
    assert_eq!(
        execute(a.id, json!(null)).await.data.into_json().unwrap()["project"]["reports"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!execute(b.id, json!(["events"])).await.errors.is_empty());
    let response = execute(a.id, json!(vec!["events"; 33])).await;
    assert_eq!(
        response.errors[0]
            .extensions
            .as_ref()
            .unwrap()
            .get("code")
            .unwrap(),
        &async_graphql::Value::from("BAD_USER_INPUT")
    );
    let response = execute(a.id, json!(["x".repeat(201)])).await;
    assert!(!response.errors.is_empty());
    storage
        .connection()
        .execute_raw(sql(
            "DELETE FROM clickman_projects WHERE id IN ($1,$2)",
            vec![a.id.into(), b.id.into()],
        ))
        .await
        .unwrap();
}
