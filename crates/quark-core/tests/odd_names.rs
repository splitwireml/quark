//! Awkward column names (a quote, a space, an accent, a keyword) work in every analysis route and export.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.
#![allow(clippy::unwrap_used)]

use std::io::Cursor;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use calamine::{Reader, Xlsx};
use duckdb::Connection;
use http_body_util::BodyExt;
use quark_core::features::sources::service::datasets;
use quark_core::features::sources::uploads::register_upload;
use quark_core::runtime::AppState;
use quark_core::runtime::dirs::Dirs;
use serde_json::{Value, json};
use tower::ServiceExt;

const NODE: &str = "odd";
const QUOTED: &str = "a\"b";
const SPACED: &str = "with space";
const ACCENTED: &str = "Zoë";
const KEYWORD: &str = "select";
const COLUMNS: [&str; 4] = [QUOTED, SPACED, ACCENTED, KEYWORD];
const ROWS: i64 = 12;

struct Fixture {
    _root: tempfile::TempDir,
    app: Router,
    dataset: String,
    table: String,
}

/// 12 rows: `a"b` is k0 to k2, `with space` is the row number, `Zoë` is z0 or z1 and `select` is 1.5 times the row.
fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let csv = root.path().join("odd.csv");
    write_csv(&csv);
    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let state = AppState::load(dirs).unwrap();
    register_upload(&state, None, NODE, "odd.csv", &csv).unwrap();
    let info = datasets(&state, NODE).unwrap().remove(0).info;
    Fixture {
        app: quark_core::router(state),
        dataset: info.id,
        table: info.name,
        _root: root,
    }
}

fn write_csv(path: &Path) {
    let target = path.to_string_lossy().replace('\'', "''");
    Connection::open_in_memory()
        .unwrap()
        .execute_batch(&format!(
            "COPY (SELECT 'k' || (i % 3) AS \"a\"\"b\", i AS \"with space\", 'z' || (i % 2) AS \"Zoë\",
                    i * 1.5 AS \"select\" FROM range({ROWS}) t(i)) TO '{target}' (HEADER)"
        ))
        .unwrap();
}

/// Percent-encodes every byte outside the unreserved set, as a client does for a path segment.
fn segment(name: &str) -> String {
    name.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

async fn send(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (u16, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    let body = body.map_or_else(Body::empty, |json| Body::from(json.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

async fn json_ok(app: &Router, method: &str, uri: &str, body: Option<Value>) -> Value {
    let (status, bytes) = send(app, method, uri, body).await;
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(status, 200, "{method} {uri}: {text}");
    serde_json::from_slice(&bytes).unwrap()
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn select_all(fixture: &Fixture) -> String {
    format!("SELECT * FROM main.{}", quoted(&fixture.table))
}

async fn export(fixture: &Fixture, format: &str) -> Vec<u8> {
    let sheet = json!({"node_id": NODE, "name": "Odd", "sql": select_all(fixture)});
    let body = json!({"format": format, "sheets": [sheet]});
    let (status, bytes) = send(&fixture.app, "POST", "/api/exports", Some(body)).await;
    assert_eq!(status, 200, "{format}: {}", String::from_utf8_lossy(&bytes));
    bytes
}

#[tokio::test]
async fn odd_column_names_work_in_every_analysis_route() {
    let fixture = fixture();
    let base = format!("/api/nodes/{NODE}/datasets/{}", fixture.dataset);

    for name in COLUMNS {
        let uri = format!("{base}/columns/{}/stats", segment(name));
        let stats = json_ok(&fixture.app, "POST", &uri, Some(json!({}))).await;
        assert_eq!(stats["row_count"], ROWS, "stats of {name}");
    }

    let uri = format!("{base}/columns/{}/values", segment(QUOTED));
    let values = json_ok(&fixture.app, "GET", &uri, None).await;
    assert_eq!(values["total"], 3);
    assert_eq!(values["values"][0], json!({"value": "k0", "count": 4}));

    let find = json!({"sql": select_all(&fixture), "term": "z1", "columns": [KEYWORD, ACCENTED]});
    let found = json_ok(
        &fixture.app,
        "POST",
        &format!("/api/nodes/{NODE}/sql/find"),
        Some(find),
    )
    .await;
    let cell = &found["match"];
    assert_eq!(
        (
            &cell["row"],
            &cell["column"],
            &cell["column_index"],
            &cell["value"]
        ),
        (&json!(1), &json!(ACCENTED), &json!(1), &json!("z1"))
    );

    let spec = json!({"chart": "bar", "metric": "sum", "encodings": {
        "category": QUOTED, "value": SPACED, "group": ACCENTED}});
    let chart = json_ok(
        &fixture.app,
        "POST",
        &format!("{base}/visualize"),
        Some(json!({"spec": spec})),
    )
    .await;
    assert_eq!(chart["series"], json!(["z0", "z1"]));
    assert_eq!(chart["rows"].as_array().unwrap().len(), 3);

    let csv = String::from_utf8(export(&fixture, "csv").await).unwrap();
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("\"a\"\"b\",with space,Zoë,select"));
    assert_eq!(lines.next(), Some("k0,0,z0,0.0"));
    assert_eq!(lines.count(), usize::try_from(ROWS - 1).unwrap());
}

#[tokio::test]
async fn odd_column_names_survive_json_xlsx_and_parquet_exports() {
    let fixture = fixture();

    let rows: Value = serde_json::from_slice(&export(&fixture, "json").await).unwrap();
    let first = rows[0].as_object().unwrap();
    assert_eq!(
        first.keys().map(String::as_str).collect::<Vec<_>>(),
        COLUMNS
    );
    assert_eq!(first[QUOTED], "k0");

    let sheet_bytes = export(&fixture, "xlsx").await;
    let mut workbook: Xlsx<_> = calamine::open_workbook_from_rs(Cursor::new(sheet_bytes)).unwrap();
    let range = workbook.worksheet_range("Odd").unwrap();
    let header: Vec<String> = range
        .rows()
        .next()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(header, COLUMNS);

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("odd.parquet");
    std::fs::write(&file, export(&fixture, "parquet").await).unwrap();
    let path = file.to_string_lossy().replace('\'', "''");
    let conn = Connection::open_in_memory().unwrap();
    let mut statement = conn
        .prepare(&format!("DESCRIBE SELECT * FROM '{path}'"))
        .unwrap();
    let names: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(names, COLUMNS);
}
