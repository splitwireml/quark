//! Tests for column stats: error precedence, GET and POST parity, and the shared cache entry.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use duckdb::Connection;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tower::ServiceExt;

use super::service::stats;
use crate::http::router;
use crate::relation::request::{Direction, Filter, QueryRequest, Sort};
use crate::relation::target::Target;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;

/// A started app with one CSV node `legacy` (dataset `main.data`, columns a and b).
fn app(root: &std::path::Path) -> Router {
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    fs::create_dir_all(&dirs.data).unwrap();
    let file = root.join("legacy.csv");
    fs::write(&file, "a,b\n1,x\n2,y\n3,z\n").unwrap();
    let entry = json!([{
        "id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy(),
    }]);
    fs::write(dirs.registry_file(), entry.to_string()).unwrap();
    fs::write(dirs.projects_file(), "[]").unwrap();
    router(AppState::load(dirs).unwrap())
}

fn stats_uri(column: &str) -> String {
    format!(
        "/api/nodes/legacy/datasets/{}/columns/{column}/stats",
        dataset_id("main", "data")
    )
}

async fn send(app: &Router, method: &str, uri: &str, body: Option<&str>) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |text| Body::from(text.to_owned()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    (
        parts.status.as_u16(),
        serde_json::from_slice(&bytes).unwrap(),
    )
}

#[tokio::test]
async fn error_precedence_matches_python() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let bad_filter = r#"{"filters":[{"column":"nope","operator":"=","value":1}]}"#;

    let (status, body) = send(&app, "POST", &stats_uri("ghost"), Some(bad_filter)).await;
    assert_eq!(status, 422);
    assert_eq!(body, json!({"detail": "Invalid filter column or operator"}));
    let (status, body) = send(&app, "POST", &stats_uri("ghost"), None).await;
    assert_eq!((status, body), (404, json!({"detail": "Column not found"})));
    let (status, body) = send(
        &app,
        "GET",
        "/api/nodes/legacy/datasets/bm9wZQ/columns/a/stats",
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (404, json!({"detail": "Dataset not found"}))
    );
    let (status, body) = send(
        &app,
        "GET",
        "/api/nodes/nope/datasets/bm9wZQ/columns/a/stats",
        None,
    )
    .await;
    assert_eq!((status, body), (404, json!({"detail": "Node not found"})));
    let (status, _) = send(&app, "POST", &stats_uri("a"), Some(r#"{"page":0}"#)).await;
    assert_eq!(status, 422);
}

#[tokio::test]
async fn get_equals_post_with_empty_body() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let uri = stats_uri("a");

    let (status, got) = send(&app, "GET", &uri, None).await;
    assert_eq!(status, 200);
    assert_eq!(got["kind"], "numeric");
    assert_eq!(got["row_count"], 3);
    for body in [None, Some("{}")] {
        assert_eq!(send(&app, "POST", &uri, body).await, (200, got.clone()));
    }
    let sql = json!({"sql": "SELECT b FROM main.data", "filters": [
        {"column": "b", "operator": "=", "value": "x"}]});
    let (status, sql_stats) = send(
        &app,
        "POST",
        "/api/nodes/legacy/sql/columns/b/stats",
        Some(&sql.to_string()),
    )
    .await;
    assert_eq!((status, &sql_stats["row_count"]), (200, &json!(1)));
}

fn memory_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t AS SELECT * FROM range(5) r(a)")
        .unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn profile(engine: &Engine, request: &QueryRequest) -> Value {
    let target = Target::Sql("SELECT * FROM t".to_owned());
    engine
        .read(None, |conn| stats(engine, conn, &target, request, "a"))
        .unwrap()
        .unwrap()
}

#[test]
fn sorts_and_paging_share_one_cache_entry() {
    let engine = memory_engine();
    let first = profile(&engine, &QueryRequest::default());
    engine
        .primary()
        .conn
        .execute_batch("INSERT INTO t VALUES (99)")
        .unwrap();

    let reordered = QueryRequest {
        page: 3,
        page_size: 10,
        sorts: vec![Sort {
            column: "a".to_owned(),
            direction: Direction::Desc,
        }],
        ..QueryRequest::default()
    };
    assert_eq!(profile(&engine, &reordered), first);

    let narrowed = QueryRequest {
        filters: vec![Filter {
            column: "a".to_owned(),
            operator: ">".to_owned(),
            value: Some(json!(1)),
            connector: Default::default(),
        }],
        ..QueryRequest::default()
    };
    assert_eq!(profile(&engine, &narrowed)["row_count"], 4);
}
