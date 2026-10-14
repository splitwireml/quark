//! Tests for find: direction and wrap-around, table reuse, eviction and column checks.

use std::sync::Barrier;
use std::thread;

use duckdb::Connection;

use super::model::{CellSearch, SearchDirection};
use super::service::{find_cell, prepare_memory, source_for};
use crate::runtime::cache::numbered::NumberedState;
use crate::runtime::engine::{Engine, EngineKind};

fn engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t AS SELECT i AS id,
            CASE WHEN i IN (2, 7) THEN '100% O''Brien' ELSE 'plain' END AS note FROM range(10) r(i)",
    )
    .unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn search(sql: &str, term: &str, columns: &[&str]) -> CellSearch {
    CellSearch {
        sql: sql.to_owned(),
        term: term.to_owned(),
        columns: columns.iter().map(|name| (*name).to_owned()).collect(),
        after_row: -1,
        after_column: -1,
        direction: SearchDirection::Next,
    }
}

/// The row a search finds, or None.
fn row_of(engine: &Engine, request: &CellSearch) -> Option<i64> {
    let response = engine
        .read(None, |conn| find_cell(engine, conn, request))
        .unwrap()
        .unwrap();
    response.found.map(|cell| cell.row)
}

fn numbered_tables(engine: &Engine) -> Vec<String> {
    let primary = engine.primary();
    let mut statement = primary
        .conn
        .prepare("SELECT table_name FROM duckdb_tables() WHERE table_name LIKE 'f\\_%' ESCAPE '\\' ORDER BY 1")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn finds_next_and_previous_and_wraps() {
    let engine = engine();
    let mut request = search("SELECT * FROM t ORDER BY id", "o'BRIEN", &["note", "id"]);
    assert_eq!(row_of(&engine, &request), Some(2));
    request.after_row = 2;
    request.after_column = 0;
    assert_eq!(row_of(&engine, &request), Some(7));
    request.after_row = 7;
    assert_eq!(row_of(&engine, &request), Some(2));
    request.direction = SearchDirection::Previous;
    request.after_row = 2;
    assert_eq!(row_of(&engine, &request), Some(7));

    let mut request = search("SELECT * FROM t ORDER BY id", "%", &["note"]);
    assert_eq!(row_of(&engine, &request), Some(2));
    request.term = "not present".to_owned();
    assert_eq!(row_of(&engine, &request), None);
    let reported = engine
        .read(None, |conn| {
            find_cell(
                &engine,
                conn,
                &search("SELECT * FROM t ORDER BY id", "7", &["id", "note"]),
            )
        })
        .unwrap()
        .unwrap()
        .found
        .unwrap();
    assert_eq!(
        (
            reported.row,
            reported.column.as_str(),
            reported.column_index
        ),
        (7, "id", 0)
    );
    assert_eq!(reported.value, "7");
}

#[test]
fn second_search_reuses_the_numbered_table() {
    let engine = engine();
    let sql = "SELECT i, random()::VARCHAR AS r FROM range(5) t(i)";
    let first = engine
        .read(None, |conn| {
            find_cell(&engine, conn, &search(sql, ".", &["r"]))
        })
        .unwrap()
        .unwrap()
        .found
        .unwrap();
    // A rebuilt table would hold different random values, so the exact text only matches a reused one.
    let again = search(sql, &first.value, &["r"]);
    assert_eq!(row_of(&engine, &again), Some(first.row));
    assert_eq!(numbered_tables(&engine).len(), 1);
}

#[test]
fn fifth_view_evicts_the_oldest_table() {
    let engine = engine();
    for limit in 1..=5 {
        let sql = format!("SELECT * FROM t LIMIT {limit}");
        assert_eq!(row_of(&engine, &search(&sql, "plain", &["note"])), Some(0));
    }
    assert_eq!(numbered_tables(&engine).len(), 4);
    let oldest = engine.numbered().touch("SELECT * FROM t LIMIT 1", []);
    assert_eq!(
        oldest.state,
        NumberedState::Create,
        "the first View's table was evicted"
    );
}

#[test]
fn invalid_column_is_422() {
    let engine = engine();
    let unknown = search("SELECT * FROM t", "x", &["missing"]);
    let repeated = search("SELECT * FROM t", "x", &["id", "id"]);
    for request in [unknown, repeated] {
        let error = engine
            .read(None, |conn| find_cell(&engine, conn, &request))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.status().as_u16(), 422);
        assert_eq!(error.detail(), "Invalid search column");
    }
    let write = search("DELETE FROM t", "x", &["id"]);
    let error = engine
        .read(None, |conn| find_cell(&engine, conn, &write))
        .unwrap()
        .unwrap_err();
    assert_eq!(error.status().as_u16(), 422);
}

#[test]
fn search_reads_the_live_relation_while_another_search_builds_the_table() {
    let engine = engine();
    let sql = "SELECT * FROM t ORDER BY id";
    let creator = engine.numbered().touch(sql, ["id", "note"]);
    assert_eq!(creator.state, NumberedState::Create);
    assert_eq!(row_of(&engine, &search(sql, "o'brien", &["note"])), Some(2));
    assert!(
        numbered_tables(&engine).is_empty(),
        "only the creator makes the table"
    );
    assert_eq!(
        engine.numbered().touch(sql, ["id", "note"]).state,
        NumberedState::Building
    );
    assert!(engine.numbered().mark_ready(&creator.table));
    assert_eq!(
        engine.numbered().touch(sql, ["id", "note"]).state,
        NumberedState::Ready
    );
}

#[test]
fn failed_build_is_created_again_by_the_next_search() {
    let engine = engine();
    let sql = "SELECT * FROM t";
    let creator = engine.numbered().touch(sql, ["id", "note"]);
    engine.numbered().forget(&creator.table);
    assert_eq!(
        engine.numbered().touch(sql, ["id", "note"]).state,
        NumberedState::Create
    );
}

#[test]
fn creator_drops_the_table_it_built_when_its_entry_was_evicted() {
    let engine = engine();
    let sql = "SELECT * FROM t ORDER BY id";
    let creator = engine.numbered().touch(sql, ["id", "note"]);
    for limit in 1..=4 {
        engine
            .numbered()
            .touch(&format!("SELECT * FROM t LIMIT {limit}"), []);
    }
    // Eviction found nothing to drop while the table was uncommitted, so the late creator cleans up itself.
    let source = engine
        .read(None, |conn| source_for(&engine, conn, &creator, sql))
        .unwrap()
        .unwrap();
    assert_eq!(numbered_tables(&engine).len(), 1);
    let orphan = source.orphan.unwrap();
    assert_eq!(orphan, creator.table);
    prepare_memory(&engine, &[orphan]).unwrap();
    assert!(numbered_tables(&engine).is_empty());
}

#[test]
fn concurrent_searches_of_one_unseen_view_both_find_the_match() {
    let engine = engine();
    for round in 0..3 {
        let sql = format!("SELECT i, 'v' || i AS s FROM range(3000000) r(i) WHERE i >= {round}");
        let request = search(&sql, "v2999999", &["s"]);
        let barrier = Barrier::new(2);
        let rows: Vec<_> = thread::scope(|scope| {
            let workers: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        engine
                            .read(None, |conn| find_cell(&engine, conn, &request))
                            .unwrap()
                            .map(|response| response.found.map(|cell| cell.row))
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect()
        });
        for found in rows {
            assert_eq!(found.unwrap(), Some(2_999_999 - round));
        }
        assert_eq!(
            engine.numbered().touch(&sql, ["i", "s"]).state,
            NumberedState::Ready
        );
    }
}

#[test]
fn forgetting_a_ready_entry_keeps_it() {
    let engine = engine();
    let sql = "SELECT * FROM t";
    assert_eq!(row_of(&engine, &search(sql, "plain", &["note"])), Some(0));
    let table = engine.numbered().touch(sql, ["id", "note"]).table;
    engine.numbered().forget(&table);
    assert_eq!(
        engine.numbered().touch(sql, ["id", "note"]).state,
        NumberedState::Ready
    );
}

#[test]
fn row_key_never_collides_with_a_user_column() {
    let engine = engine();
    let sql = "SELECT * FROM clash";
    let key = engine.numbered().touch(sql, []).row_key;
    engine
        .primary()
        .conn
        .execute_batch(&format!(
            "CREATE TABLE clash AS SELECT i AS id, 'needle ' || i AS \"{key}\" FROM range(3) r(i)"
        ))
        .unwrap();
    let request = search(sql, "needle 2", &[&key, "id"]);
    let found = engine
        .read(None, |conn| find_cell(&engine, conn, &request))
        .unwrap()
        .unwrap()
        .found
        .unwrap();
    assert_eq!(
        (found.row, found.column.as_str(), found.value.as_str()),
        (2, key.as_str(), "needle 2")
    );
    let shadow_free = engine.numbered().touch(sql, [key.as_str()]).row_key;
    assert_eq!(shadow_free, format!("{key}_"));
}
