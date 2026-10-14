//! Tests for running one page: paging, caching, result tables and Arrow output.

use super::*;
use crate::relation::describe::datasets;
use crate::relation::request::{Direction, Filter, Sort};
use crate::runtime::cache::results::{ResultCache, table_name};
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::registry::SourceRecord;
use crate::shared::ids::dataset_id;
use arrow::ipc::reader::StreamReader;
use duckdb::Connection;
use serde_json::json;
use std::io::Cursor;
use std::thread;
use std::time::{Duration, Instant};

mod results;

fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

/// Runs one page on a pooled reader, as the route does.
fn fetch(
    engine: &Engine,
    target: &Target,
    request: &QueryRequest,
    format: Format,
) -> ApiResult<PageBody> {
    engine
        .read(None, |conn| run_page(engine, conn, target, request, format))
        .unwrap()
}

fn dataset(name: &str) -> Target {
    Target::Dataset(dataset_id("main", name))
}

fn json_body(body: PageBody) -> Value {
    match body {
        PageBody::Json(value) => value,
        PageBody::Arrow(_) => panic!("expected a JSON body"),
    }
}

fn arrow_body(body: PageBody) -> Vec<u8> {
    match body {
        PageBody::Arrow(bytes) => bytes,
        PageBody::Json(_) => panic!("expected an Arrow body"),
    }
}

fn filter(column: &str, operator: &str, value: Value) -> Filter {
    Filter {
        column: column.to_owned(),
        operator: operator.to_owned(),
        value: Some(value),
        connector: Default::default(),
    }
}

#[test]
fn pages_count_and_offsets() {
    let engine = engine("CREATE TABLE t AS SELECT range AS a FROM range(250)");
    let sorted = |page| QueryRequest {
        page,
        sorts: vec![Sort {
            column: "a".to_owned(),
            direction: Direction::Asc,
        }],
        ..QueryRequest::default()
    };

    let second = json_body(fetch(&engine, &dataset("t"), &sorted(2), Format::Json).unwrap());
    assert_eq!(second["page"], 2);
    assert_eq!(second["page_size"], 100);
    assert_eq!(second["total_rows"], 250);
    assert_eq!(second["total_pages"], 3);
    let rows = second["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 100);
    assert_eq!(rows[0], json!({"a": 100}));
    assert!(second["elapsed_ms"].as_f64().unwrap() >= 0.0);

    let last = json_body(fetch(&engine, &dataset("t"), &sorted(3), Format::Json).unwrap());
    assert_eq!(last["rows"].as_array().unwrap().len(), 50);

    let beyond = json_body(fetch(&engine, &dataset("t"), &sorted(4), Format::Json).unwrap());
    assert_eq!(beyond["rows"], json!([]));
    assert_eq!(beyond["total_rows"], 250);
}

fn count_queries(engine: &Engine) -> u64 {
    engine.counters.count_queries.load(Ordering::Relaxed)
}

fn page_of(page: i64, sorts: Vec<Sort>, filters: Vec<Filter>) -> QueryRequest {
    QueryRequest {
        page,
        sorts,
        filters,
        ..QueryRequest::default()
    }
}

fn sort_a(direction: Direction) -> Vec<Sort> {
    vec![Sort {
        column: "a".to_owned(),
        direction,
    }]
}

#[test]
fn second_page_skips_count_and_nulls() {
    let engine = engine("CREATE TABLE t AS SELECT range AS a FROM range(250)");
    let run = |engine: &Engine, page| {
        json_body(
            fetch(
                engine,
                &dataset("t"),
                &page_of(page, sort_a(Direction::Asc), Vec::new()),
                Format::Json,
            )
            .unwrap(),
        )
    };

    let first = run(&engine, 1);
    let second = run(&engine, 2);

    assert_eq!(count_queries(&engine), 1);
    assert_eq!(second["total_rows"], 250);
    assert_eq!(second["columns"], first["columns"]);
    assert_eq!(second["rows"][0], json!({"a": 100}));
}

#[test]
fn sort_change_reuses_stats() {
    let engine = engine("CREATE TABLE t AS SELECT range AS a FROM range(10)");
    let run = |engine: &Engine, sorts| {
        json_body(
            fetch(
                engine,
                &dataset("t"),
                &page_of(1, sorts, Vec::new()),
                Format::Json,
            )
            .unwrap(),
        )
    };

    let ascending = run(&engine, sort_a(Direction::Asc));
    let descending = run(&engine, sort_a(Direction::Desc));

    assert_eq!(count_queries(&engine), 1);
    assert_eq!(ascending["rows"][0], json!({"a": 0}));
    assert_eq!(descending["rows"][0], json!({"a": 9}));
    assert_eq!(descending["total_rows"], 10);
}

#[test]
fn filter_change_misses() {
    let engine = engine("CREATE TABLE t AS SELECT range AS a FROM range(10)");
    let run = |engine: &Engine, bound| {
        json_body(
            fetch(
                engine,
                &dataset("t"),
                &page_of(1, Vec::new(), vec![filter("a", ">", json!(bound))]),
                Format::Json,
            )
            .unwrap(),
        )
    };

    let above_two = run(&engine, 2);
    let above_six = run(&engine, 6);
    let above_six_again = run(&engine, 6);

    assert_eq!(count_queries(&engine), 2);
    assert_eq!(above_two["total_rows"], 7);
    assert_eq!(above_six["total_rows"], 3);
    assert_eq!(above_six_again["total_rows"], 3);
}

#[test]
fn arrow_format_carries_the_json_metadata() {
    let engine = engine(
        "CREATE TABLE t AS SELECT range::INTEGER AS a, DATE '2024-01-02' AS d FROM range(5)",
    );
    let request = QueryRequest {
        page_size: 2,
        ..QueryRequest::default()
    };
    let mut expected = json_body(fetch(&engine, &dataset("t"), &request, Format::Json).unwrap());

    let bytes = arrow_body(fetch(&engine, &dataset("t"), &request, Format::Arrow).unwrap());

    let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
    let mut meta: Value = serde_json::from_str(&reader.schema().metadata()["quark"]).unwrap();
    let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
    assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
    assert_eq!(
        meta.as_object_mut().unwrap().remove("json_columns"),
        Some(json!(["d"]))
    );
    for body in [&mut meta, &mut expected] {
        body.as_object_mut().unwrap().remove("elapsed_ms");
    }
    expected.as_object_mut().unwrap().remove("rows");
    assert_eq!(meta, expected);
}

#[test]
fn null_fractions_per_column() {
    let engine = engine(
        "CREATE TABLE t (a INTEGER, b VARCHAR, c DATE);
         INSERT INTO t VALUES (1, 'x', NULL), (NULL, 'y', NULL), (3, 'z', NULL), (NULL, 'w', NULL)",
    );

    let body = json_body(
        fetch(
            &engine,
            &dataset("t"),
            &QueryRequest::default(),
            Format::Json,
        )
        .unwrap(),
    );

    assert_eq!(
        body["columns"],
        json!([
            {"name": "a", "type": "INTEGER", "numeric": true, "profile_kind": "numeric", "null_fraction": 0.5},
            {"name": "b", "type": "VARCHAR", "numeric": false, "profile_kind": "categorical", "null_fraction": 0.0},
            {"name": "c", "type": "DATE", "numeric": false, "profile_kind": "date", "null_fraction": 1.0},
        ])
    );
    assert_eq!(body["rows"][1], json!({"a": null, "b": "y", "c": null}));
}

#[test]
fn header_only_csv_is_an_empty_page() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("empty.csv");
    std::fs::write(&file, "a,b\n").unwrap();
    let record = SourceRecord {
        id: "s1".to_owned(),
        name: "empty".to_owned(),
        kind: "file".to_owned(),
        source: file,
        project_id: None,
        dataset_name: Some("empty".to_owned()),
        sheets: None,
    };
    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let engine = Engine::open_node(&record, &dirs).unwrap();

    let body = json_body(
        fetch(
            &engine,
            &dataset("empty"),
            &QueryRequest::default(),
            Format::Json,
        )
        .unwrap(),
    );

    assert_eq!(body["rows"], json!([]));
    assert_eq!(body["total_rows"], 0);
    assert_eq!(body["total_pages"], 0);
    let columns = body["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0]["name"], "a");
    assert_eq!(columns[1]["null_fraction"], 0.0);
}

#[test]
fn sql_target_display_rules() {
    let engine = engine("");
    let sql = Target::Sql("  select 7 as x  ".to_owned());

    let plain = json_body(fetch(&engine, &sql, &QueryRequest::default(), Format::Json).unwrap());
    assert_eq!(plain["sql"], "select 7 as x");
    assert_eq!(plain["rows"], json!([{"x": 7}]));

    let filtered = QueryRequest {
        filters: vec![filter("x", ">", json!(5))],
        ..QueryRequest::default()
    };
    let body = json_body(fetch(&engine, &sql, &filtered, Format::Json).unwrap());
    assert_eq!(
        body["sql"],
        "SELECT * FROM (SELECT * FROM (select 7 as x) WHERE \"x\" > 5)"
    );
    assert_eq!(body["total_rows"], 1);

    let sorted = QueryRequest {
        sorts: vec![Sort {
            column: "x".to_owned(),
            direction: Direction::Desc,
        }],
        ..QueryRequest::default()
    };
    let body = json_body(fetch(&engine, &sql, &sorted, Format::Json).unwrap());
    assert_eq!(
        body["sql"],
        "SELECT * FROM (SELECT * FROM (select 7 as x)) ORDER BY \"x\" DESC"
    );
}

#[test]
fn sql_target_errors_map_to_422() {
    let engine = engine("");
    let run = |engine: &Engine, sql: &str| {
        fetch(
            engine,
            &Target::Sql(sql.to_owned()),
            &QueryRequest::default(),
            Format::Json,
        )
        .unwrap_err()
    };

    let two = run(&engine, "select 1; select 2");
    assert_eq!(two.status(), 422);
    assert_eq!(two.detail(), "SQL accepts only one read-only SELECT query");

    let missing = run(&engine, "select * from missing_table");
    assert_eq!(missing.status(), 422);
    assert!(
        missing.detail().starts_with("Invalid SQL query: "),
        "{}",
        missing.detail()
    );

    let conversion = run(&engine, "select cast('x' as integer) as v");
    assert_eq!(conversion.status(), 422);
    assert!(
        conversion.detail().starts_with("Invalid SQL query: "),
        "{}",
        conversion.detail()
    );
}

#[test]
fn invalid_value_maps_to_422() {
    let engine = engine("CREATE TABLE t AS SELECT 1 AS a");
    let request = QueryRequest {
        filters: vec![filter("a", "=", json!("abc"))],
        ..QueryRequest::default()
    };

    let error = fetch(&engine, &dataset("t"), &request, Format::Json).unwrap_err();

    assert_eq!(error.status(), 422);
    assert!(
        error.detail().starts_with("Invalid filter value: "),
        "{}",
        error.detail()
    );
}

#[test]
fn unknown_dataset_is_404() {
    let engine = engine("");

    let error = fetch(
        &engine,
        &dataset("nope"),
        &QueryRequest::default(),
        Format::Json,
    )
    .unwrap_err();

    assert_eq!(error.status(), 404);
    assert_eq!(error.detail(), "Dataset not found");
}
