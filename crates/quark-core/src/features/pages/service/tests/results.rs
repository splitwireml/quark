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

fn wait_until_ready(inner: &mut EngineInner, key: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !matches!(inner.results.note_request(key), ResultAction::ServeFrom(_)) {
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
            let mut fresh = inner(ROWS);
            let request = sorted_by_id(*page, Direction::Asc);
            without_elapsed(run_page(&mut fresh, &dataset("t"), &request, Format::Json).unwrap())
        })
        .collect();
    let mut inner = inner(ROWS);
    let first = sorted_by_id(1, Direction::Asc);
    run_page(&mut inner, &dataset("t"), &first, Format::Json).unwrap();
    run_page(&mut inner, &dataset("t"), &first, Format::Json).unwrap();
    wait_until_ready(&mut inner, &id_key("t", "ASC"));
    // Direct reads now see different values, so equal pages must come from the result table.
    inner.conn.execute_batch("UPDATE t SET h = -1").unwrap();

    for (page, expected) in pages.iter().zip(direct) {
        let request = sorted_by_id(*page, Direction::Asc);
        let body = run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
        assert_eq!(without_elapsed(body), expected, "page {page}");
    }
    assert_eq!(count_queries(&inner), 1);
}

#[test]
fn result_tables_are_not_datasets() {
    let mut inner = inner(ROWS);
    let request = sorted_by_id(1, Direction::Asc);
    run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
    run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
    wait_until_ready(&mut inner, &id_key("t", "ASC"));
    assert_eq!(result_tables(&inner.conn), 1);

    let names: Vec<String> = datasets(&inner.conn)
        .unwrap()
        .into_iter()
        .map(|item| item.name)
        .collect();

    assert_eq!(names, ["t"]);
}

#[test]
fn plain_scans_build_no_result() {
    let mut inner = inner(ROWS);

    for _ in 0..3 {
        let request = QueryRequest::default();
        run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
    }

    assert!(!has_results_schema(&inner.conn));
}

#[test]
fn evicted_results_are_dropped() {
    let mut inner = inner(ROWS);
    inner.results = ResultCache::with_capacity(1);
    let ascending = sorted_by_id(1, Direction::Asc);
    run_page(&mut inner, &dataset("t"), &ascending, Format::Json).unwrap();
    run_page(&mut inner, &dataset("t"), &ascending, Format::Json).unwrap();
    wait_until_ready(&mut inner, &id_key("t", "ASC"));
    assert_eq!(result_tables(&inner.conn), 1);

    let descending = sorted_by_id(1, Direction::Desc);
    run_page(&mut inner, &dataset("t"), &descending, Format::Json).unwrap();
    assert_eq!(result_tables(&inner.conn), 1);
    run_page(&mut inner, &dataset("t"), &descending, Format::Json).unwrap();

    let ascending_table = table_name(&id_key("t", "ASC"));
    assert_eq!(
        count(
            &inner.conn,
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
    let first = Engine::open_node(&record, &dirs, 1).unwrap();
    {
        let mut inner = first.lock();
        run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();
        run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();
        wait_until_ready(&mut inner, &id_key("rows", "ASC"));
        assert_eq!(result_tables(&inner.conn), 1);
    }

    drop(first);
    let second = Engine::open_node(&record, &dirs, 2).unwrap();
    let mut inner = second.lock();
    assert_eq!(result_tables(&inner.conn), 0);
    run_page(&mut inner, &dataset("rows"), &request, Format::Json).unwrap();

    assert_eq!(count_queries(&inner), 1);
    assert_eq!(result_tables(&inner.conn), 0);
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
    let engine = Engine::open_node(&record, &dirs, 1).unwrap();
    let mut inner = engine.lock();
    let request = sorted_by_id(1, Direction::Asc);
    let first = run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
    run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();

    wait_until_ready(&mut inner, &id_key("t", "ASC"));

    assert_eq!(result_tables(&inner.conn), 1);
    let third = run_page(&mut inner, &dataset("t"), &request, Format::Json).unwrap();
    assert_eq!(without_elapsed(third), without_elapsed(first));
    assert_eq!(count_queries(&inner), 1);
    let names: Vec<String> = datasets(&inner.conn)
        .unwrap()
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(names, ["t"]);
}
