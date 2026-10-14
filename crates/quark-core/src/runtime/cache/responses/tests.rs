//! Tests for the response cache, driven through `cached_response` on real engines.

use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use serde_json::{Value, json};

use super::*;
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::EngineKind;
use crate::runtime::registry::SourceRecord;

const STORED_ELAPSED: f64 = 999.0;

fn dirs_in(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

fn csv_engine(root: &Path, body: &str) -> Engine {
    let file = root.join("x.csv");
    fs::write(&file, body).unwrap();
    let record = SourceRecord {
        id: "s1".to_owned(),
        name: "s1".to_owned(),
        kind: "file".to_owned(),
        source: file,
        project_id: None,
        dataset_name: Some("data".to_owned()),
        sheets: None,
    };
    Engine::open_node(&record, &dirs_in(root)).unwrap()
}

fn memory_engine() -> Engine {
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, Connection::open_in_memory().unwrap()).unwrap()
}

fn relation(sql: &str, params: Vec<Value>) -> Relation {
    Relation {
        sql: sql.to_owned(),
        params,
        columns: Vec::new(),
    }
}

fn plain() -> Relation {
    relation("(SELECT * FROM \"data\")", Vec::new())
}

/// Counts the calls that reach `compute`.
struct Counter(Cell<u32>);

impl Counter {
    fn new() -> Self {
        Self(Cell::new(0))
    }

    fn ask(&self, engine: &Engine, relation: &Relation, detail: &Value) -> Value {
        self.ask_kind(engine, "stats", relation, detail)
    }

    fn ask_kind(&self, engine: &Engine, kind: &str, relation: &Relation, detail: &Value) -> Value {
        engine
            .read(None, |conn| {
                engine.cached_response(conn, kind, relation, detail, || {
                    self.0.set(self.0.get() + 1);
                    Ok(json!({"rows": self.0.get(), "elapsed_ms": STORED_ELAPSED}))
                })
            })
            .unwrap()
            .unwrap()
    }

    fn calls(&self) -> u32 {
        self.0.get()
    }
}

#[test]
fn repeat_requests_reuse_the_response() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();

    let first = counter.ask(&engine, &plain(), &json!({"column": "a"}));
    let second = counter.ask(&engine, &plain(), &json!({"column": "a"}));

    assert_eq!(counter.calls(), 1);
    assert_eq!(first["rows"], second["rows"]);
    assert_eq!(first["elapsed_ms"], STORED_ELAPSED);
    let elapsed = second["elapsed_ms"].as_f64().unwrap();
    assert!(elapsed < STORED_ELAPSED);
    assert!(((elapsed * 1000.0).round() - elapsed * 1000.0).abs() < 1e-6);
}

#[test]
fn filters_columns_and_sql_get_their_own_entries() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a,b\n1,2\n");
    let counter = Counter::new();
    let filtered = relation("(SELECT * FROM \"data\" WHERE \"a\" > ?)", vec![json!(0)]);
    let other_value = relation("(SELECT * FROM \"data\" WHERE \"a\" > ?)", vec![json!(5)]);
    let sql = relation("(SELECT * FROM query(?))", vec![json!("SELECT 1 AS a")]);
    let column_a = json!({"column": "a"});
    let requests = [
        (plain(), column_a.clone(), "stats"),
        (plain(), json!({"column": "b"}), "stats"),
        (plain(), column_a.clone(), "chart"),
        (filtered, column_a.clone(), "stats"),
        (other_value, column_a.clone(), "stats"),
        (sql, column_a, "stats"),
    ];

    for (relation, detail, kind) in &requests {
        counter.ask_kind(&engine, kind, relation, detail);
    }
    assert_eq!(counter.calls(), 6);
    for (relation, detail, kind) in &requests {
        counter.ask_kind(&engine, kind, relation, detail);
    }
    assert_eq!(counter.calls(), 6);
}

#[test]
fn in_place_edit_of_the_source_misses() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();
    counter.ask(&engine, &plain(), &Value::Null);

    sleep(Duration::from_millis(20));
    fs::write(root.path().join("x.csv"), "a\n2\n").unwrap();
    counter.ask(&engine, &plain(), &Value::Null);

    assert_eq!(counter.calls(), 2);
}

#[test]
fn missing_source_file_is_never_cached() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();
    fs::remove_file(root.path().join("x.csv")).unwrap();

    counter.ask(&engine, &plain(), &Value::Null);
    counter.ask(&engine, &plain(), &Value::Null);

    assert_eq!(counter.calls(), 2);
}

#[test]
fn volatile_and_dynamic_sql_is_never_cached() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();
    let nested = relation(
        "(SELECT * FROM query(?))",
        vec![json!("SELECT * FROM query('SELECT 1')")],
    );
    let requests = [
        relation("(SELECT * FROM \"data\" WHERE random() < 2)", Vec::new()),
        relation("(SELECT *, current_timestamp FROM \"data\")", Vec::new()),
        relation("(SELECT * FROM \"data\" USING SAMPLE 1 ROWS)", Vec::new()),
        nested,
    ];

    for relation in &requests {
        counter.ask(&engine, relation, &Value::Null);
        counter.ask(&engine, relation, &Value::Null);
    }

    assert_eq!(counter.calls(), 8);
}

#[test]
fn stored_views_and_macros_in_attached_databases_disable_caching() {
    let root = tempfile::tempdir().unwrap();
    let with_view = root.path().join("views.duckdb");
    let with_macro = root.path().join("macros.duckdb");
    Connection::open(&with_view)
        .unwrap()
        .execute_batch("CREATE VIEW stored AS SELECT 1 AS a")
        .unwrap();
    Connection::open(&with_macro)
        .unwrap()
        .execute_batch("CREATE MACRO plus_one(x) AS x + 1")
        .unwrap();
    let uses_macro = relation("(SELECT plus_one(1) AS a)", Vec::new());
    let plain_select = relation("(SELECT 1 AS a)", Vec::new());

    let macros = memory_engine();
    let counter = Counter::new();
    macros
        .primary()
        .conn
        .execute_batch(&format!(
            "ATTACH '{}' AS ext (READ_ONLY)",
            with_macro.display()
        ))
        .unwrap();
    counter.ask(&macros, &plain_select, &Value::Null);
    counter.ask(&macros, &plain_select, &Value::Null);
    assert_eq!(counter.calls(), 1);
    counter.ask(&macros, &uses_macro, &Value::Null);
    counter.ask(&macros, &uses_macro, &Value::Null);
    assert_eq!(counter.calls(), 3);

    let views = memory_engine();
    let counter = Counter::new();
    views
        .primary()
        .conn
        .execute_batch(&format!(
            "ATTACH '{}' AS ext (READ_ONLY)",
            with_view.display()
        ))
        .unwrap();
    counter.ask(&views, &plain_select, &Value::Null);
    counter.ask(&views, &plain_select, &Value::Null);
    assert_eq!(counter.calls(), 2);
}

#[test]
fn direct_file_scans_are_never_cached() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();
    let requests = [
        relation("(SELECT * FROM read_csv('x.csv'))", Vec::new()),
        relation("(SELECT * FROM \"data\")", vec![json!("other.parquet")]),
        relation(
            "(SELECT * FROM \"data\")",
            vec![json!({"path": "Data.XLSX"})],
        ),
    ];

    for relation in &requests {
        counter.ask(&engine, relation, &Value::Null);
        counter.ask(&engine, relation, &Value::Null);
    }

    assert_eq!(counter.calls(), 6);
}

#[test]
fn least_recently_used_entry_goes_at_128() {
    let root = tempfile::tempdir().unwrap();
    let engine = csv_engine(root.path(), "a\n1\n");
    let counter = Counter::new();
    for index in 0..=CAPACITY {
        counter.ask(&engine, &plain(), &json!({"n": index}));
    }
    assert_eq!(counter.calls(), 129);

    counter.ask(&engine, &plain(), &json!({"n": 1}));
    assert_eq!(counter.calls(), 129);
    counter.ask(&engine, &plain(), &json!({"n": 0}));
    assert_eq!(counter.calls(), 130);
}
