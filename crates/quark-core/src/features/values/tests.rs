//! Tests for category values: counting, ordering, search, paging, validation and the HTTP routes.

use super::service::{ValuesPage, category_values};
use crate::error::ApiResult;
use crate::http::router;
use crate::relation::target::Target;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use duckdb::Connection;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tower::ServiceExt;

fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn items() -> Engine {
    engine(
        "CREATE TABLE items(category VARCHAR, amount INTEGER);
         INSERT INTO items VALUES ('beta', 1), ('beta', 2), ('O''Reilly', 3),
           (''' OR true --', 4), (NULL, 5), ('Alpha', 6)",
    )
}

fn many() -> Engine {
    engine("CREATE TABLE items AS SELECT printf('value-%03d', i) AS category FROM range(601) t(i)")
}

fn page(search: &str, offset: i64, limit: i64) -> ValuesPage {
    ValuesPage {
        search: search.to_owned(),
        offset,
        limit,
    }
}

fn values(engine: &Engine, target: &Target, column: &str, page: &ValuesPage) -> ApiResult<Value> {
    engine
        .read(None, |conn| {
            category_values(engine, conn, target, column, page)
        })
        .unwrap()
}

fn items_table() -> Target {
    Target::Dataset(dataset_id("main", "items"))
}

#[test]
fn values_are_counted_and_ordered() {
    let body = values(&items(), &items_table(), "category", &page("", 0, 200)).unwrap();
    assert_eq!(
        body,
        json!({
            "values": [
                {"value": "beta", "count": 2},
                {"value": "' OR true --", "count": 1},
                {"value": "Alpha", "count": 1},
                {"value": "O'Reilly", "count": 1},
            ],
            "total": 4,
            "offset": 0,
            "limit": 200,
            "has_more": false,
        })
    );
}

#[test]
fn sql_targets_are_counted_the_same_way() {
    let target = Target::Sql("SELECT category FROM items WHERE amount < 4".to_owned());
    let body = values(&items(), &target, "category", &page("", 0, 200)).unwrap();
    assert_eq!(body["total"], 2);
    assert_eq!(body["values"][0], json!({"value": "beta", "count": 2}));
}

#[test]
fn search_is_case_insensitive_and_literal() {
    let engine = items();
    let body = values(&engine, &items_table(), "category", &page("ALPH", 0, 200)).unwrap();
    assert_eq!(body["values"], json!([{"value": "Alpha", "count": 1}]));
    assert_eq!(body["total"], 1);
    let quote = values(
        &engine,
        &items_table(),
        "category",
        &page("' OR true --", 0, 200),
    )
    .unwrap();
    assert_eq!(
        quote["values"],
        json!([{"value": "' OR true --", "count": 1}])
    );
}

#[test]
fn paging_reports_has_more() {
    let engine = many();
    let table = items_table();
    let first = values(&engine, &table, "category", &page("", 0, 200)).unwrap();
    assert_eq!(first["values"].as_array().unwrap().len(), 200);
    assert_eq!(
        (&first["total"], &first["has_more"]),
        (&json!(601), &json!(true))
    );
    let last = values(&engine, &table, "category", &page("", 600, 200)).unwrap();
    assert_eq!(last["values"], json!([{"value": "value-600", "count": 1}]));
    assert_eq!(last["has_more"], false);
    let searched = values(&engine, &table, "category", &page("VALUE-59", 0, 3)).unwrap();
    assert_eq!(
        (&searched["total"], &searched["has_more"]),
        (&json!(10), &json!(true))
    );
}

#[test]
fn empty_page_past_the_end_still_reports_the_total() {
    let body = values(&many(), &items_table(), "category", &page("", 5000, 200)).unwrap();
    assert_eq!(
        body,
        json!({"values": [], "total": 601, "offset": 5000, "limit": 200, "has_more": false})
    );
    let none = values(&many(), &items_table(), "category", &page("absent", 0, 200)).unwrap();
    assert_eq!((&none["values"], &none["total"]), (&json!([]), &json!(0)));
}

#[test]
fn non_text_column_is_422() {
    let error = values(&items(), &items_table(), "amount", &page("", 0, 200)).unwrap_err();
    assert_eq!(error.status(), 422);
    assert_eq!(error.detail(), "Column is not text");
}

#[test]
fn missing_column_or_dataset_is_404() {
    let engine = items();
    let column = values(&engine, &items_table(), "absent", &page("", 0, 200)).unwrap_err();
    assert_eq!(
        (column.status().as_u16(), column.detail()),
        (404, "Column not found")
    );
    let target = Target::Dataset(dataset_id("main", "absent"));
    let dataset = values(&engine, &target, "category", &page("", 0, 200)).unwrap_err();
    assert_eq!(
        (dataset.status().as_u16(), dataset.detail()),
        (404, "Dataset not found")
    );
}

/// A started app with one CSV node `legacy`: dataset `main.data`, a text column `b` of 100 values.
fn app(root: &std::path::Path) -> Router {
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    fs::create_dir_all(&dirs.data).unwrap();
    let file = root.join("legacy.csv");
    let rows: String = (0..100).map(|i| format!("{i},value-{i}\n")).collect();
    fs::write(&file, format!("a,b\n{rows}")).unwrap();
    let entry = json!([{
        "id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy(),
    }]);
    fs::write(dirs.registry_file(), entry.to_string()).unwrap();
    fs::write(dirs.projects_file(), "[]").unwrap();
    router(AppState::load(dirs).unwrap())
}

fn url(column: &str, query: &str) -> String {
    let dataset = dataset_id("main", "data");
    format!("/api/nodes/legacy/datasets/{dataset}/columns/{column}/values{query}")
}

async fn send(app: &Router, request: Request<Body>) -> (u16, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn get(app: &Router, uri: &str) -> (u16, Value) {
    send(app, Request::get(uri).body(Body::empty()).unwrap()).await
}

#[tokio::test]
async fn dataset_route_serves_values_and_rejects_bad_paging() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let (status, body) = get(&app, &url("b", "?search=VALUE-9&limit=3")).await;
    assert_eq!(status, 200);
    assert_eq!(body["total"], 11);
    assert_eq!(body["values"].as_array().unwrap().len(), 3);
    for query in ["?offset=-1", "?limit=0", "?limit=501", "?limit=many"] {
        let (status, _) = get(&app, &url("b", query)).await;
        assert_eq!(status, 422, "{query}");
    }
    let (status, body) = get(&app, &url("a", "")).await;
    assert_eq!(
        (status, &body["detail"]),
        (422, &json!("Column is not text"))
    );
    let (status, _) = get(&app, &url("missing", "")).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn sql_route_serves_values_from_the_body() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let post = |body: Value, query: &str| {
        Request::post(format!("/api/nodes/legacy/sql/columns/b/values{query}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let sql = json!({"sql": "SELECT b FROM data WHERE a < 5"});
    let (status, body) = send(&app, post(sql, "?search=value-1")).await;
    assert_eq!((status, &body["total"]), (200, &json!(1)));
    let (status, _) = send(&app, post(json!({"sql": "SELECT b FROM data"}), "?limit=0")).await;
    assert_eq!(status, 422);
}

/// Each request reads on its own pooled connection: DuckDB connections are not shared between
/// simultaneous requests (the Rust counterpart of the Python-only shared-cursor test).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn category_values_do_not_share_a_connection_concurrently() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let mut tasks = tokio::task::JoinSet::new();
    for search in 0..8 {
        let app = app.clone();
        tasks.spawn(async move { get(&app, &url("b", &format!("?search={search}"))).await });
    }
    let mut totals = Vec::new();
    while let Some(done) = tasks.join_next().await {
        let (status, body) = done.unwrap();
        assert_eq!(status, 200);
        totals.push(body["total"].as_u64().unwrap());
    }
    assert_eq!(totals.len(), 8);
    assert!(totals.iter().all(|total| *total >= 10));
}
