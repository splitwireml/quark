//! Tests for opening nodes, recovering the primary lock and running reads side by side.

use super::*;
use crate::relation::describe::datasets;
use crate::shared::ids::dataset_id;
use crate::shared::sql::{scan_expression, sql_string};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn count_rows(conn: &Connection, file: &Path) -> duckdb::Result<i64> {
    let scan = scan_expression(&file.to_string_lossy()).unwrap();
    conn.query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
        row.get(0)
    })
}

fn dirs_in(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

fn record(file: &Path, dataset_name: Option<&str>) -> SourceRecord {
    SourceRecord {
        id: "s1".to_owned(),
        name: "s1".to_owned(),
        kind: "file".to_owned(),
        source: file.to_path_buf(),
        project_id: None,
        dataset_name: dataset_name.map(str::to_owned),
        sheets: None,
    }
}

fn open(file: &Path, dataset_name: Option<&str>, root: &Path) -> anyhow::Result<Engine> {
    Engine::open_node(&record(file, dataset_name), &dirs_in(root))
}

fn names(engine: &Engine) -> Vec<(String, String, String)> {
    datasets(&engine.primary().conn)
        .unwrap()
        .into_iter()
        .map(|item| (item.schema, item.name, item.kind))
        .collect()
}

fn main_view(name: &str) -> Vec<(String, String, String)> {
    vec![("main".to_owned(), name.to_owned(), "VIEW".to_owned())]
}

#[test]
fn node_datasets_per_format() {
    let root = tempfile::tempdir().unwrap();
    let json = r#"[{"a":1}]"#;
    let lines = "{\"a\":1}\n";
    for (file, body) in [
        ("x.csv", "a\n1\n"),
        ("x.tsv", "a\n1\n"),
        ("x.json", json),
        ("x.ndjson", lines),
        ("x.jsonl", lines),
    ] {
        let path = root.path().join(file);
        fs::write(&path, body).unwrap();
        let engine = open(&path, Some("x"), root.path()).unwrap();
        assert_eq!(names(&engine), main_view("x"), "{file}");
    }
    let parquet = root.path().join("x.parquet");
    Connection::open_in_memory()
        .unwrap()
        .execute_batch(&format!(
            "COPY (SELECT 1 AS a) TO {} (FORMAT parquet)",
            sql_string(&parquet.to_string_lossy())
        ))
        .unwrap();
    let engine = open(&parquet, Some("x"), root.path()).unwrap();
    assert_eq!(names(&engine), main_view("x"));

    let database = root.path().join("x.duckdb");
    Connection::open(&database)
        .unwrap()
        .execute_batch("CREATE TABLE items (a INTEGER)")
        .unwrap();
    let engine = open(&database, None, root.path()).unwrap();
    assert_eq!(
        names(&engine),
        vec![("main".to_owned(), "items".to_owned(), "TABLE".to_owned())]
    );
    assert_eq!(
        datasets(&engine.primary().conn).unwrap()[0].id,
        dataset_id("main", "items")
    );
}

#[test]
fn duckdb_sources_are_read_only() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("db")).unwrap();
    let database = root.path().join("db").join("x.db");
    Connection::open(&database)
        .unwrap()
        .execute_batch("CREATE TABLE items (a INTEGER)")
        .unwrap();
    let outside = root.path().join("elsewhere").join("outside.csv");
    fs::create_dir_all(outside.parent().unwrap()).unwrap();
    fs::write(&outside, "a\n1\n").unwrap();

    let engine = open(&database, None, root.path()).unwrap();
    let guard = engine.primary();

    assert!(
        guard
            .conn
            .execute_batch("INSERT INTO items VALUES (1)")
            .is_err()
    );
    assert!(
        guard
            .conn
            .execute_batch("CREATE TABLE more (a INTEGER)")
            .is_err()
    );
    assert!(count_rows(&guard.conn, &outside).is_err());
}

#[test]
fn database_node_blocks_every_file_even_beside_it() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("db");
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir_all(&folder).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    let database = folder.join("x.duckdb");
    Connection::open(&database)
        .unwrap()
        .execute_batch("CREATE TABLE items (a INTEGER)")
        .unwrap();
    let beside = folder.join("beside.csv");
    let outside = elsewhere.join("outside.csv");
    fs::write(&beside, "a\n1\n2\n").unwrap();
    fs::write(&outside, "a\n1\n").unwrap();

    let engine = open(&database, None, root.path()).unwrap();
    let guard = engine.primary();

    assert!(count_rows(&guard.conn, &beside).is_err());
    assert!(count_rows(&guard.conn, &outside).is_err());
}

#[test]
fn paths_with_quotes_spaces_and_unicode() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("it's a dir").join("Zoë data📊");
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join("Zoë's claims (v1).csv");
    fs::write(&file, "a\n1\n2\n").unwrap();

    let engine = open(&file, Some("we\"ird 'name'"), root.path()).unwrap();
    let guard = engine.primary();

    let rows: i64 = guard
        .conn
        .query_row("SELECT count(*) FROM \"we\"\"ird 'name'\"", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 2);
    assert_eq!(datasets(&guard.conn).unwrap()[0].name, "we\"ird 'name'");
}

#[test]
fn poisoned_engine_lock_recovers() {
    let engine = std::sync::Arc::new(
        Engine::new(
            EngineKind::Node {
                node_id: "s1".to_owned(),
            },
            Connection::open_in_memory().unwrap(),
        )
        .unwrap(),
    );
    let held = std::sync::Arc::clone(&engine);
    let joined = std::thread::spawn(move || {
        let _guard = held.primary();
        panic!("poison the engine lock");
    })
    .join();
    assert!(joined.is_err());
    assert!(engine.primary.is_poisoned());

    let one: i64 = engine
        .primary()
        .conn
        .query_row("SELECT 1", [], |row| row.get(0))
        .unwrap();

    assert_eq!(one, 1);
}

#[test]
fn open_node_rejects_excel_and_unknown_files() {
    let root = tempfile::tempdir().unwrap();
    assert!(open(&root.path().join("x.xlsx"), None, root.path()).is_err());
    assert!(open(&root.path().join("x.txt"), None, root.path()).is_err());
}

/// Single-threaded, so concurrent reads can only be faster than serial ones by running on separate readers.
const SLOW_QUERY: &str =
    "SELECT count(*) FROM range(3000) a(x), range(10000) b(y) WHERE (x + y) % 7 = 0";

fn slow_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("SET threads = 1").unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn read_slow(engine: &Engine) -> Duration {
    let started = Instant::now();
    let total: i64 = engine
        .read(None, |conn| {
            conn.query_row(SLOW_QUERY, [], |row| row.get(0))
        })
        .unwrap()
        .unwrap();
    assert!(total > 0);
    started.elapsed()
}

#[test]
fn pages_of_one_engine_run_concurrently() {
    let engine = slow_engine();
    let single = read_slow(&engine);

    let started = Instant::now();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2).map(|_| scope.spawn(|| read_slow(&engine))).collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
    let together = started.elapsed();

    // Serial reads would take about twice one read.
    assert!(
        together < single.mul_f64(1.5),
        "two reads took {together:?}, one takes {single:?}"
    );
}
