//! Tests for cross-source join engines: what they read, how many stay open and which is dropped.

use super::{JOIN_LIMIT, JoinSlots};
use crate::relation::describe::DatasetInfo;
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::registry::SourceRecord;
use crate::runtime::state::fixtures::*;
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use rust_xlsxwriter::Workbook;
use serde_json::json;
use std::sync::Arc;

fn engine(id: &str) -> Arc<Engine> {
    let kind = EngineKind::Node {
        node_id: id.to_owned(),
    };
    Arc::new(Engine::new(kind, Connection::open_in_memory().unwrap()).unwrap())
}

fn data_item() -> DatasetInfo {
    DatasetInfo {
        id: dataset_id("main", "data"),
        name: "data".to_owned(),
        schema: "main".to_owned(),
        kind: "VIEW".to_owned(),
    }
}

fn source(state: &crate::runtime::AppState, id: &str) -> SourceRecord {
    let catalog = state.catalog();
    catalog.sources.iter().find(|s| s.id == id).unwrap().clone()
}

fn count(engine: &Engine, sql: &str) -> duckdb::Result<i64> {
    engine
        .read(None, |conn| conn.query_row(sql, [], |row| row.get(0)))
        .unwrap()
}

#[test]
fn join_engine_reads_columnar_copies_when_ready() {
    let root = tempfile::tempdir().unwrap();
    let registry = [
        entry(root.path(), "a1", None),
        entry(root.path(), "b1", None),
    ];
    let state = started_with_worker(root.path(), &registry, &[]);
    let (left, right) = (source(&state, "a1"), source(&state, "b1"));
    state.queue_import(&left);
    state.queue_import(&right);
    wait_until("both caches", || cache_files(&state.0.dirs).len() == 2);

    let (id, engine) = state
        .open_join((&left, &data_item()), (&right, &data_item()))
        .unwrap();

    assert!(id.starts_with("join_"));
    let cached = "SELECT count(*) FROM duckdb_views() WHERE schema_name LIKE '%_source' AND contains(sql, 'cache_')";
    assert_eq!(count(&engine, cached).unwrap(), 2);
    let rows =
        r#"SELECT count(*) FROM "left_source"."data" l JOIN "right_source"."data" r USING (a)"#;
    assert_eq!(count(&engine, rows).unwrap(), 2);
    for file in cache_files(&state.0.dirs) {
        assert!(engine.source_files.contains(&file));
    }
    assert!(!engine.source_files.contains(&left.source));
}

#[test]
fn join_engine_reads_a_workbook_side_from_its_cache() {
    let root = tempfile::tempdir().unwrap();
    let book = root.path().join("book.xlsx");
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Sales").unwrap();
    sheet.write_string(0, 0, "a").unwrap();
    sheet.write_string(0, 1, "total").unwrap();
    for (row, value) in (1u32..).zip([1.0, 2.0, 3.0]) {
        sheet.write_number(row, 0, value).unwrap();
        sheet.write_number(row, 1, value * 10.0).unwrap();
    }
    workbook.save(&book).unwrap();
    let workbook_entry = json!({
        "id": "w1", "name": "W1", "kind": "xlsx", "source": book.to_string_lossy(),
        "sheets": ["Sales"],
    });
    let registry = [workbook_entry, entry(root.path(), "b1", None)];
    let state = started(root.path(), &registry, &[]);
    let (left, right) = (source(&state, "w1"), source(&state, "b1"));
    let sales = DatasetInfo {
        id: dataset_id("main", "Sales"),
        name: "Sales".to_owned(),
        schema: "main".to_owned(),
        kind: "VIEW".to_owned(),
    };

    let (_, engine) = state
        .open_join((&left, &sales), (&right, &data_item()))
        .unwrap();

    let cached = "SELECT count(*) FROM duckdb_views() WHERE schema_name = 'left_source' AND contains(sql, 'cache_')";
    assert_eq!(count(&engine, cached).unwrap(), 1);
    let rows = r#"SELECT sum(l.total)::BIGINT FROM "left_source"."Sales" l JOIN "right_source"."data" r USING (a)"#;
    assert_eq!(count(&engine, rows).unwrap(), 30);
    let caches = cache_files(&state.0.dirs);
    assert_eq!(caches.len(), 1);
    assert!(engine.source_files.contains(&caches[0]));
    assert!(engine.source_files.contains(&right.source));
    assert!(!engine.source_files.contains(&left.source));
    let escape = format!(
        "SELECT count(*) FROM read_xlsx('{}')",
        left.source.to_string_lossy().replace('\'', "''")
    );
    assert!(count(&engine, &escape).is_err());
}

#[test]
fn join_engine_cannot_read_other_files() {
    let root = tempfile::tempdir().unwrap();
    let registry = [
        entry(root.path(), "a1", None),
        entry(root.path(), "b1", None),
    ];
    let state = started(root.path(), &registry, &[]);
    let other = root.path().join("other.csv");
    write(&other, "a\n1\n");
    let (left, right) = (source(&state, "a1"), source(&state, "b1"));

    let (_, engine) = state
        .open_join((&left, &data_item()), (&right, &data_item()))
        .unwrap();

    let rows = r#"SELECT count(*) FROM "left_source"."data""#;
    assert_eq!(count(&engine, rows).unwrap(), 2);
    assert!(engine.source_files.contains(&left.source));
    assert!(engine.source_files.contains(&right.source));
    let escape = format!(
        "SELECT count(*) FROM read_csv_auto('{}')",
        other.to_string_lossy().replace('\'', "''")
    );
    assert!(count(&engine, &escape).is_err());
}

#[test]
fn fifth_join_evicts_the_least_recent() {
    let mut slots = JoinSlots::default();
    for number in 1..=JOIN_LIMIT {
        let id = format!("join_{number}");
        assert!(slots.insert(id.clone(), engine(&id)).is_none());
    }
    // Looking a join up makes it the most recent, so the next one in line is evicted.
    assert!(slots.get("join_1").is_some());
    let evicted = slots.insert("join_5".to_owned(), engine("join_5"));

    assert!(evicted.is_some());
    assert!(slots.get("join_2").is_none());
    for kept in ["join_1", "join_3", "join_4", "join_5"] {
        assert!(slots.get(kept).is_some(), "{kept} stays open");
    }
}

#[test]
fn an_evicted_join_engine_finishes_its_running_queries() {
    let mut slots = JoinSlots::default();
    let first = engine("join_1");
    slots.insert("join_1".to_owned(), Arc::clone(&first));
    for number in 2..=JOIN_LIMIT + 1 {
        let id = format!("join_{number}");
        slots.insert(id.clone(), engine(&id));
    }

    assert!(slots.get("join_1").is_none());
    assert_eq!(count(&first, "SELECT 7").unwrap(), 7);
}
