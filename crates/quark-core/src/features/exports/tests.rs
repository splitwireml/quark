//! Tests for the export route: CSV downloads, cleanup of the temporary file, the sheet rules and the startup sweep.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, header};
use duckdb::Connection;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::model::{ExportFormat, ExportSheet, JsonLayout};
use super::service::{prepare_sheet, write_export};
use crate::http::router;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::{Engine, EngineKind};

fn dirs(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

/// A started app with one CSV node `legacy`: table `data` with integer `a` (0 to 5999) and text `b`.
pub(super) fn app(root: &Path) -> Router {
    let dirs = dirs(root);
    fs::create_dir_all(&dirs.data).unwrap();
    let file = root.join("legacy.csv");
    let rows: String = (0..6000).map(|i| format!("{i},value-{i}\n")).collect();
    fs::write(&file, format!("a,b\n{rows}")).unwrap();
    let entry = json!([{
        "id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy(),
    }]);
    fs::write(dirs.registry_file(), entry.to_string()).unwrap();
    fs::write(dirs.projects_file(), "[]").unwrap();
    router(AppState::load(dirs).unwrap())
}

pub(super) fn exports_in(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(dirs(root).cache.join("exports"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

pub(super) fn sheet(node_id: &str, sql: &str) -> Value {
    json!({"node_id": node_id, "name": "Sheet", "sql": sql})
}

pub(super) async fn export(app: &Router, body: Value) -> Response<Body> {
    let request = Request::post("/api/exports")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.clone().oneshot(request).await.unwrap()
}

async fn csv_export(app: &Router, sql: &str) -> Response<Body> {
    export(
        app,
        json!({"format": "csv", "sheets": [sheet("legacy", sql)]}),
    )
    .await
}

pub(super) async fn text(response: Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub(super) async fn failure(response: Response<Body>) -> (u16, String) {
    let status = response.status().as_u16();
    let raw = text(response).await;
    let body: Value = serde_json::from_str(&raw).expect(&raw);
    (status, body["detail"].as_str().unwrap().to_owned())
}

#[tokio::test]
async fn csv_export_writes_headers_and_rows() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let request = json!({
        "format": "csv",
        "filename": "../../all-data",
        "sheets": [sheet("legacy", "SELECT b, a FROM data ORDER BY a")],
    });

    let response = export(&app, request).await;

    assert_eq!(response.status(), 200);
    let headers = response.headers();
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"all-data.csv\""
    );
    let body = text(response).await;
    let mut lines = body.split("\r\n");
    assert_eq!(lines.next(), Some("b,a"));
    assert_eq!(lines.next(), Some("value-0,0"));
    assert_eq!(body.matches("\r\n").count(), 6001);
    assert!(body.ends_with("value-5999,5999\r\n"));
    assert!(exports_in(root.path()).is_empty());
}

#[tokio::test]
async fn csv_export_neutralizes_formulas_and_quotes_fields() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(
        &app,
        "SELECT * FROM (VALUES ('=1+1', 1.5), ('@SUM(1,1)', NULL), ('say \"hi\"', 2)) v(note, n)",
    )
    .await;

    assert_eq!(
        text(response).await,
        "note,n\r\n'=1+1,1.5\r\n\"'@SUM(1,1)\",\r\n\"say \"\"hi\"\"\",2.0\r\n"
    );
}

#[tokio::test]
async fn csv_export_keeps_repeated_column_names_in_the_header() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(&app, "SELECT 1 AS a, 2 AS a").await;

    assert_eq!(text(response).await, "a,a\r\n1,2\r\n");
}

#[tokio::test]
async fn csv_export_keeps_timetz_offsets() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(&app, "SELECT TIMETZ '10:30:00+05:30' AS t").await;

    assert_eq!(text(response).await, "t\r\n10:30:00+05:30\r\n");
}

#[tokio::test]
async fn abandoned_export_leaves_no_file() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(&app, "SELECT * FROM data").await;

    assert_eq!(response.status(), 200);
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap();
    assert!(first.is_data());
    assert_eq!(exports_in(root.path()).len(), 1);
    drop(body);
    assert!(exports_in(root.path()).is_empty());
}

#[tokio::test]
async fn finished_export_leaves_no_file() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(&app, "SELECT * FROM data").await;
    let _ = text(response).await;

    assert!(exports_in(root.path()).is_empty());
}

#[tokio::test]
async fn empty_results_export_valid_files() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let response = csv_export(&app, "SELECT a, b FROM data WHERE false").await;

    assert_eq!(response.status(), 200);
    assert_eq!(text(response).await, "a,b\r\n");
}

#[tokio::test]
async fn export_needs_exactly_one_sheet_for_csv() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let one = sheet("legacy", "SELECT 1");
    for (format, label) in [("csv", "CSV"), ("parquet", "PARQUET"), ("json", "JSON")] {
        let two = json!({"format": format, "sheets": [one, one]});
        assert_eq!(
            failure(export(&app, two).await).await,
            (422, format!("{label} export requires exactly one sheet"))
        );
        let none = json!({"format": format, "sheets": []});
        assert_eq!(failure(export(&app, none).await).await.0, 422);
    }
    let many = json!({"format": "xlsx", "sheets": vec![one; 101]});
    assert_eq!(failure(export(&app, many).await).await.0, 422);
    assert!(exports_in(root.path()).is_empty());
}

#[tokio::test]
async fn export_rejects_unknown_nodes_and_unrunnable_sql_without_a_file() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let elsewhere = tempfile::tempdir().unwrap();
    let secret = elsewhere.path().join("secret.txt");
    fs::write(&secret, "private").unwrap();
    let read_text = format!("SELECT content FROM read_text('{}')", secret.display());
    for sql in [
        "DELETE FROM data",
        "SELECT 1; SELECT 2",
        "SELECT * FROM missing_table",
        read_text.as_str(),
    ] {
        for format in ["csv", "xlsx"] {
            let request = json!({"format": format, "sheets": [sheet("legacy", sql)]});
            assert_eq!(failure(export(&app, request).await).await.0, 422, "{sql}");
        }
    }
    let unknown = json!({"format": "csv", "sheets": [sheet("missing", "SELECT 1")]});
    assert_eq!(
        failure(export(&app, unknown).await).await,
        (404, "Node not found".to_owned())
    );
    let failed = csv_export(&app, "SELECT * FROM missing_table").await;
    let (status, detail) = failure(failed).await;
    assert_eq!(status, 422);
    assert!(detail.starts_with("Invalid SQL query: "), "{detail}");
    assert!(exports_in(root.path()).is_empty());
}

/// The Rust counterpart of the Python-only `test_export_and_metadata_are_serialized`: Python
/// serializes both on one shared connection, while here each request reads on its own pooled
/// connection, so an export and a metadata read in flight together both succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn export_and_metadata_run_together_on_separate_readers() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let metadata = {
        let app = app.clone();
        tokio::spawn(async move {
            let request = Request::post("/api/nodes/legacy/sql")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"sql": "SELECT count(*) AS n FROM data"}).to_string(),
                ))
                .unwrap();
            app.oneshot(request).await.unwrap()
        })
    };

    let exported = csv_export(&app, "SELECT * FROM range(5001) r(i)").await;

    assert_eq!(exported.status(), 200);
    assert_eq!(text(exported).await.matches("\r\n").count(), 5002);
    assert_eq!(metadata.await.unwrap().status(), 200);
}

#[test]
fn startup_sweeps_old_exports() {
    let root = tempfile::tempdir().unwrap();
    let dirs = dirs(root.path());
    let exports = dirs.cache.join("exports");
    fs::create_dir_all(&exports).unwrap();
    fs::write(exports.join(".export-abandoned.csv"), "a\r\n1\r\n").unwrap();

    AppState::load(dirs).unwrap();

    assert!(exports.is_dir());
    assert!(exports_in(root.path()).is_empty());
}

#[test]
fn write_export_writes_the_only_sheet_of_a_flat_format() {
    let root = tempfile::tempdir().unwrap();
    let engine = Arc::new(
        Engine::new(
            EngineKind::Node {
                node_id: "n".to_owned(),
            },
            Connection::open_in_memory().unwrap(),
        )
        .unwrap(),
    );
    let request = ExportSheet {
        node_id: "n".to_owned(),
        name: "Sheet".to_owned(),
        sql: "SELECT 7 AS a".to_owned(),
    };
    let prepared = prepare_sheet(&Connection::open_in_memory().unwrap(), &request).unwrap();

    let file = write_export(
        ExportFormat::Csv,
        JsonLayout::default(),
        &[(engine, prepared)],
        root.path(),
    )
    .unwrap();

    assert_eq!(fs::read_to_string(file.path()).unwrap(), "a\r\n7\r\n");
}
