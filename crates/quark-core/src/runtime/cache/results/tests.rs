//! Tests for the result cache bookkeeping and its background builds.

use super::*;
use crate::relation::request::{Connector, Filter};
use crate::runtime::cache::stats::stats_key;
use serde_json::json;
use std::thread;
use std::time::{Duration, Instant};

/// Runs for far longer than any test, so it only ends when interrupted.
const LONG_BUILD: &str = "SELECT sum(i) AS total FROM range(1000000000000) t(i)";

fn key(sql: &str) -> String {
    stats_key(sql, &[json!(1)])
}

fn view(sql: &str) -> ViewInfo {
    ViewInfo {
        id: "v".to_owned(),
        project_id: "p".to_owned(),
        source_id: "s".to_owned(),
        source_name: "sales".to_owned(),
        node_id: "n".to_owned(),
        name: "sales".to_owned(),
        schema: "main".to_owned(),
        kind: "view".to_owned(),
        columns: vec!["a".to_owned()],
        sql: sql.to_owned(),
    }
}

#[test]
fn first_request_serves_direct_second_starts_build() {
    let mut cache = ResultCache::default();
    let k = key("SELECT 1");

    assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
    let ResultAction::StartBuild(table) = cache.note_request(&k) else {
        panic!("second request should start a build");
    };
    assert_eq!(table, table_name(&k));
    assert_eq!(table.len(), 18);
    assert!(table.starts_with("r_"));
    assert!(table[2..].bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
}

#[test]
fn only_one_build_is_in_flight() {
    let mut cache = ResultCache::default();
    let (a, b) = (key("SELECT a"), key("SELECT b"));
    cache.note_request(&a);
    cache.note_request(&b);

    assert!(matches!(
        cache.note_request(&a),
        ResultAction::StartBuild(_)
    ));
    assert!(matches!(
        cache.note_request(&b),
        ResultAction::StartBuild(_)
    ));

    assert_eq!(cache.note_request(&a), ResultAction::ServeDirect);
    assert_eq!(cache.note_request(&b), ResultAction::ServeDirect);
}

#[test]
fn ready_entries_serve_from_table() {
    let mut cache = ResultCache::default();
    let k = key("SELECT 1");
    cache.note_request(&k);
    cache.note_request(&k);

    cache.mark_ready(&k);

    assert_eq!(
        cache.note_request(&k),
        ResultAction::ServeFrom(table_name(&k))
    );
}

#[test]
fn capacity_evicts_and_returns_dropped_table() {
    let mut cache = ResultCache::with_capacity(2);
    let (a, b, c, d) = (key("a"), key("b"), key("c"), key("d"));
    trigger(&mut cache, &a);
    cache.mark_ready(&a);
    trigger(&mut cache, &b);
    cache.mark_ready(&b);
    assert!(cache.take_dropped().is_empty());

    trigger(&mut cache, &c);
    assert_eq!(cache.take_dropped(), vec![table_name(&a)]);
    cache.mark_ready(&c);

    trigger(&mut cache, &d);
    assert_eq!(cache.take_dropped(), vec![table_name(&b)]);
    assert_eq!(
        drop_statement(&table_name(&a)),
        format!("DROP TABLE IF EXISTS {RESULTS_SCHEMA}.{}", table_name(&a))
    );
}

#[test]
fn one_off_queries_do_not_evict_results() {
    let mut cache = ResultCache::default();
    let kept: Vec<String> = (0..4).map(|i| key(&format!("kept {i}"))).collect();
    for k in &kept {
        trigger(&mut cache, k);
        cache.mark_ready(k);
    }

    for i in 0..10 {
        assert_eq!(
            cache.note_request(&key(&format!("one-off {i}"))),
            ResultAction::ServeDirect
        );
    }

    assert!(cache.take_dropped().is_empty());
    for k in &kept {
        assert_eq!(
            cache.note_request(k),
            ResultAction::ServeFrom(table_name(k))
        );
    }
}

#[test]
fn default_capacity_is_four() {
    let mut cache = ResultCache::default();
    let keys: Vec<String> = (0..5).map(|i| key(&format!("result {i}"))).collect();
    for k in &keys[..4] {
        trigger(&mut cache, k);
        cache.mark_ready(k);
    }
    assert!(cache.take_dropped().is_empty());

    trigger(&mut cache, &keys[4]);
    cache.mark_ready(&keys[4]);

    assert_eq!(cache.take_dropped(), vec![table_name(&keys[0])]);
    for k in &keys[1..] {
        assert_eq!(
            cache.note_request(k),
            ResultAction::ServeFrom(table_name(k))
        );
    }
    assert_eq!(cache.note_request(&keys[0]), ResultAction::ServeDirect);
}

#[test]
fn plain_scans_are_skipped() {
    let views = [view("SELECT * FROM sales")];
    let plain = QueryRequest::default();
    let filtered = QueryRequest {
        filters: vec![Filter {
            column: "a".to_owned(),
            operator: "=".to_owned(),
            value: Some(json!(1)),
            connector: Connector::default(),
        }],
        ..QueryRequest::default()
    };

    assert!(is_plain_scan(&plain, "SELECT * FROM sales", &views));
    assert!(!is_plain_scan(&plain, "SELECT 2", &views));
    assert!(!is_plain_scan(&filtered, "SELECT * FROM sales", &views));
}

fn wait_until(mut is_done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !is_done() {
        assert!(Instant::now() < deadline, "timed out waiting");
        thread::sleep(Duration::from_millis(10));
    }
}

fn state_of(cache: &ResultCache, key: &str) -> Option<State> {
    lock(&cache.entries)
        .iter()
        .find(|(known, _)| known == key)
        .map(|(_, state)| *state)
}

fn result_tables(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT count(*) FROM duckdb_tables() WHERE schema_name = 'quark_results'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

/// Two requests for `key`: the second one's table name.
fn trigger(cache: &mut ResultCache, key: &str) -> String {
    cache.note_request(key);
    let ResultAction::StartBuild(table) = cache.note_request(key) else {
        panic!("second request should start a build");
    };
    table
}

#[test]
fn second_request_builds_while_pages_stay_direct() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = ResultCache::default();
    let sql = "SELECT i, hash(i) AS h FROM range(3000000) t(i) WHERE i < ? ORDER BY h";
    let params = vec![json!(2_999_990)];
    let k = stats_key(sql, &params);
    let table = trigger(&mut cache, &k);

    cache.start_build(&conn, &k, &table, sql, params);

    assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
    wait_until(|| cache.note_request(&k) == ResultAction::ServeFrom(table.clone()));
    let rows: i64 = conn
        .query_row(
            &format!("SELECT count(*) FROM {RESULTS_SCHEMA}.{table}"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 2_999_990);
}

#[test]
fn superseded_build_is_interrupted() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = ResultCache::default();
    let (a, b) = (key("a"), key("b"));
    cache.note_request(&b);
    let table_a = trigger(&mut cache, &a);
    cache.start_build(&conn, &a, &table_a, LONG_BUILD, Vec::new());
    let ResultAction::StartBuild(table_b) = cache.note_request(&b) else {
        panic!("second request should start a build");
    };
    cache.start_build(&conn, &b, &table_b, LONG_BUILD, Vec::new());

    assert_eq!(state_of(&cache, &a), Some(State::Failed));
    // The cache and build `b` hold the entries; build `a` lets go once interrupted.
    wait_until(|| Arc::strong_count(&cache.entries) == 2);
    assert_eq!(state_of(&cache, &b), Some(State::Building));
    assert_eq!(result_tables(&conn), 0);
}

#[test]
fn dropping_the_cache_interrupts_a_running_build() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = ResultCache::default();
    let k = key("long");
    let table = trigger(&mut cache, &k);
    cache.start_build(&conn, &k, &table, LONG_BUILD, Vec::new());
    let entries = Arc::clone(&cache.entries);

    drop(cache);

    assert_eq!(Arc::strong_count(&entries), 1);
}

#[test]
fn failed_build_is_not_retried() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = ResultCache::default();
    let k = key("missing");
    let table = trigger(&mut cache, &k);

    cache.start_build(&conn, &k, &table, "SELECT * FROM missing_table", Vec::new());

    wait_until(|| state_of(&cache, &k) == Some(State::Failed));
    for _ in 0..3 {
        assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
    }
    assert_eq!(result_tables(&conn), 0);
}

#[test]
fn evicting_a_building_entry_interrupts_it() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = ResultCache::with_capacity(1);
    let (a, b) = (key("a"), key("b"));
    let table_a = trigger(&mut cache, &a);
    cache.start_build(&conn, &a, &table_a, LONG_BUILD, Vec::new());
    assert_eq!(state_of(&cache, &a), Some(State::Building));

    trigger(&mut cache, &b);

    assert_ne!(state_of(&cache, &a), Some(State::Building));
    // Only the cache still holds the entries once build `a` lets go.
    wait_until(|| Arc::strong_count(&cache.entries) == 1);
    assert_eq!(result_tables(&conn), 0);
}

#[test]
fn seen_keys_beyond_the_cap_are_forgotten() {
    let mut cache = ResultCache::default();
    let first = key("first");
    cache.note_request(&first);
    for i in 0..SEEN_CAPACITY {
        cache.note_request(&key(&format!("one-off {i}")));
    }

    assert_eq!(cache.note_request(&first), ResultAction::ServeDirect);
    assert_eq!(
        cache.note_request(&first),
        ResultAction::StartBuild(table_name(&first))
    );
}
