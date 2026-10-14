//! Tests for the result cache: building, reuse, eviction and rebuilds.

use super::*;

const ROWS: &str = "CREATE TABLE t AS SELECT range AS id, range * 7 AS h FROM range(1000)";

fn sorted_by_id(page: i64, direction: Direction) -> QueryRequest {
    QueryRequest {
        page,
        page_size: 10,
        sorts: vec![Sort {
            column: "id".to_owned(),
            direction,
        }],
        ..QueryRequest::default()
    }
}

fn without_elapsed(body: PageBody) -> Value {
    let mut body = json_body(body);
    body.as_object_mut().unwrap().remove("elapsed_ms");
    body
}

/// The ordered SQL of `sorted_by_id` over `main.<table>`, which keys the result cache.
fn id_key(table: &str, direction: &str) -> String {
    stats_key(
        &format!(r#"SELECT * FROM (SELECT * FROM "main"."{table}") ORDER BY "id" {direction}"#),
        &[],
    )
}

fn wait_until_ready(engine: &Engine, key: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !matches!(
        engine.results().note_request(key),
        ResultAction::ServeFrom(_)
    ) {
        assert!(Instant::now() < deadline, "the result never became ready");
        thread::sleep(Duration::from_millis(10));
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn result_tables(conn: &Connection) -> i64 {
    count(
        conn,
        "SELECT count(*) FROM duckdb_tables() WHERE schema_name = 'quark_results'",
    )
}

fn has_results_schema(conn: &Connection) -> bool {
    count(
        conn,
        "SELECT count(*) FROM duckdb_schemas() WHERE schema_name = 'quark_results'",
    ) > 0
}

#[test]
fn pages_from_results_equal_direct_pages() {
    let pages = [1, 2, 100];
    let direct: Vec<Value> = pages
        .iter()
        .map(|page| {
            let fresh = engine(ROWS);
            let request = sorted_by_id(*page, Direction::Asc);
            without_elapsed(fetch(&fresh, &dataset("t"), &request, Format::Json).unwrap())
        })
        .collect();
    let engine = engine(ROWS);
    let first = sorted_by_id(1, Direction::Asc);
    fetch(&engine, &dataset("t"), &first, Format::Json).unwrap();
    fetch(&engine, &dataset("t"), &first, Format::Json).unwrap();
    wait_until_ready(&engine, &id_key("t", "ASC"));
    // Direct reads now see different values, so equal pages must come from the result table.
    engine
        .primary()
        .conn
        .execute_batch("UPDATE t SET h = -1")
        .unwrap();

    for (page, expected) in pages.iter().zip(direct) {
        let request = sorted_by_id(*page, Direction::Asc);
        let body = fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
        assert_eq!(without_elapsed(body), expected, "page {page}");
    }
    assert_eq!(count_queries(&engine), 1);
}

#[test]
fn result_tables_are_not_datasets() {
    let engine = engine(ROWS);
    let request = sorted_by_id(1, Direction::Asc);
    fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
    fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
    wait_until_ready(&engine, &id_key("t", "ASC"));
    assert_eq!(result_tables(&engine.primary().conn), 1);

    let names: Vec<String> = datasets(&engine.primary().conn)
        .unwrap()
        .into_iter()
        .map(|item| item.name)
        .collect();

    assert_eq!(names, ["t"]);
}

#[test]
fn plain_scans_build_no_result() {
    let engine = engine(ROWS);

    for _ in 0..3 {
        let request = QueryRequest::default();
        fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
    }

    assert!(!has_results_schema(&engine.primary().conn));
}

#[test]
fn evicted_results_are_dropped() {
    let engine = engine(ROWS);
    *engine.results() = ResultCache::with_capacity(1);
    let ascending = sorted_by_id(1, Direction::Asc);
    fetch(&engine, &dataset("t"), &ascending, Format::Json).unwrap();
    fetch(&engine, &dataset("t"), &ascending, Format::Json).unwrap();
    wait_until_ready(&engine, &id_key("t", "ASC"));
    assert_eq!(result_tables(&engine.primary().conn), 1);

    let descending = sorted_by_id(1, Direction::Desc);
    fetch(&engine, &dataset("t"), &descending, Format::Json).unwrap();
    assert_eq!(result_tables(&engine.primary().conn), 1);
    fetch(&engine, &dataset("t"), &descending, Format::Json).unwrap();

    let ascending_table = table_name(&id_key("t", "ASC"));
    assert_eq!(
        count(
            &engine.primary().conn,
            &format!("SELECT count(*) FROM duckdb_tables() WHERE table_name = '{ascending_table}'"),
        ),
        0
    );
}

#[test]
fn engine_rebuild_drops_results() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("rows.csv");
    let csv: String = (0..50).map(|id| format!("{id}\n")).collect();
    std::fs::write(&file, format!("id\n{csv}")).unwrap();
    let record = SourceRecord {
        id: "s1".to_owned(),
        name: "rows".to_owned(),
        kind: "file".to_owned(),
        source: file,
        project_id: None,
        dataset_name: Some("rows".to_owned()),
        sheets: None,
    };
    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let request = sorted_by_id(1, Direction::Asc);
    let first = Engine::open_node(&record, &dirs).unwrap();
    fetch(&first, &dataset("rows"), &request, Format::Json).unwrap();
    fetch(&first, &dataset("rows"), &request, Format::Json).unwrap();
    wait_until_ready(&first, &id_key("rows", "ASC"));
    assert_eq!(result_tables(&first.primary().conn), 1);

    drop(first);
    let second = Engine::open_node(&record, &dirs).unwrap();
    assert_eq!(result_tables(&second.primary().conn), 0);
    fetch(&second, &dataset("rows"), &request, Format::Json).unwrap();

    assert_eq!(count_queries(&second), 1);
    assert_eq!(result_tables(&second.primary().conn), 0);
}

#[test]
fn database_node_builds_result_cache() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("rows.duckdb");
    Connection::open(&database)
        .unwrap()
        .execute_batch(ROWS)
        .unwrap();
    let record = SourceRecord {
        id: "s1".to_owned(),
        name: "rows".to_owned(),
        kind: "file".to_owned(),
        source: database,
        project_id: None,
        dataset_name: None,
        sheets: None,
    };
    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let engine = Engine::open_node(&record, &dirs).unwrap();
    let request = sorted_by_id(1, Direction::Asc);
    let first = fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
    fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();

    wait_until_ready(&engine, &id_key("t", "ASC"));

    assert_eq!(result_tables(&engine.primary().conn), 1);
    let third = fetch(&engine, &dataset("t"), &request, Format::Json).unwrap();
    assert_eq!(without_elapsed(third), without_elapsed(first));
    assert_eq!(count_queries(&engine), 1);
    let names: Vec<String> = datasets(&engine.primary().conn)
        .unwrap()
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(names, ["t"]);
}

/// Single-threaded, so a page over it stays busy for hundreds of milliseconds in both its count and its rows.
const SLOW_QUERY: &str =
    "SELECT count(*) FROM range(3000) a(x), range(10000) b(y) WHERE (x + y) % 7 = 0";

#[test]
fn cache_locks_are_free_while_a_page_runs() {
    let engine = engine("SET threads = 1");
    let target = Target::Sql(SLOW_QUERY.to_owned());
    let request = QueryRequest::default();

    let (page_time, slowest_lock, samples) = thread::scope(|scope| {
        let page = scope.spawn(|| {
            let started = Instant::now();
            let body = fetch(&engine, &target, &request, Format::Json).unwrap();
            assert_eq!(json_body(body)["total_rows"], 1);
            started.elapsed()
        });
        let mut slowest = Duration::ZERO;
        let mut samples = 0;
        while !page.is_finished() {
            let waited = Instant::now();
            drop(engine.stats());
            drop(engine.results());
            drop(engine.primary());
            slowest = slowest.max(waited.elapsed());
            samples += 1;
            thread::sleep(Duration::from_millis(5));
        }
        (page.join().unwrap(), slowest, samples)
    });

    assert!(page_time > Duration::from_millis(150), "{page_time:?}");
    assert!(samples >= 10, "only {samples} checks ran during the page");
    assert!(
        slowest_lock < Duration::from_millis(50),
        "a cache lock was unavailable for {slowest_lock:?} during a {page_time:?} page"
    );
}
