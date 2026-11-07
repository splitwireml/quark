//! Tests for the result cache: building, reuse, eviction and rebuilds.

use super::*;
use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId};
use duckdb::vtab::{BindInfo, InitInfo, TableFunctionInfo, VTab};
use std::error::Error;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Sender};

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

/// One pause of `paused_scan()`: where it paused, and the handle that resumes it.
type Pause = (&'static str, Sender<()>);

/// Each pause of `paused_scan()` lands here.
static PAUSES: Mutex<Option<Sender<Pause>>> = Mutex::new(None);

/// How long a paused query waits for the test before going on without it. A lock held across
/// the query keeps the test from resuming in time; a free one is taken within microseconds.
const GIVE_UP: Duration = Duration::from_secs(5);

/// Holds the calling query until the test resumes it, or `GIVE_UP` passes.
fn pause(kind: &'static str) {
    let (resume, resumed) = mpsc::channel();
    let pauses = PAUSES.lock().unwrap().clone();
    if pauses.is_some_and(|pauses| pauses.send((kind, resume)).is_ok()) {
        // Dropping `resumed` on return makes a late resume fail, which the test reports.
        let _ = resumed.recv_timeout(GIVE_UP);
    }
}

/// A one-row table function that pauses each time DuckDB binds it and each time a query scans it.
struct PausedScan;

impl VTab for PausedScan {
    type BindData = ();
    type InitData = AtomicBool;

    fn bind(bind: &BindInfo) -> Result<(), Box<dyn Error>> {
        bind.add_result_column("x", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        pause("bind");
        Ok(())
    }

    fn init(_: &InitInfo) -> Result<AtomicBool, Box<dyn Error>> {
        Ok(AtomicBool::new(false))
    }

    fn func(
        scan: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> Result<(), Box<dyn Error>> {
        if scan.get_init_data().swap(true, Ordering::Relaxed) {
            output.set_len(0);
        } else {
            pause("scan");
            output.flat_vector(0).insert(0, "row");
            output.set_len(1);
        }
        Ok(())
    }
}

/// While the page sits paused inside each of its queries, the test takes all three locks before
/// resuming it. This proves the locks are free during the queries without depending on how long
/// the page or any sleep takes.
#[test]
fn cache_locks_are_free_while_a_page_runs() {
    let engine = engine("");
    let (pauses, paused) = mpsc::channel();
    *PAUSES.lock().unwrap() = Some(pauses);
    let target = Target::Sql("SELECT * FROM paused_scan()".to_owned());
    let request = QueryRequest::default();

    let (body, checks) = thread::scope(|scope| {
        let page = scope.spawn(|| {
            engine.read(None, |conn| {
                conn.register_table_function::<PausedScan>("paused_scan")
                    .unwrap();
                run_page(&engine, conn, &target, &request, Format::Json)
            })
        });
        let mut checks = Vec::new();
        loop {
            // Read before waiting, so no pause sent before the page finished is missed.
            let finished = page.is_finished();
            match paused.recv_timeout(Duration::from_millis(10)) {
                Ok((kind, resume)) => {
                    let waited = Instant::now();
                    drop(engine.stats());
                    drop(engine.results());
                    drop(engine.primary());
                    let waited = waited.elapsed();
                    checks.push((kind, waited, resume.send(()).is_ok()));
                }
                Err(_) if finished => break,
                Err(_) => {}
            }
        }
        (page.join().unwrap(), checks)
    });
    *PAUSES.lock().unwrap() = None;

    assert_eq!(json_body(body.unwrap().unwrap())["total_rows"], 1);
    let scans = checks.iter().filter(|(kind, ..)| *kind == "scan").count();
    assert!(
        scans >= 2,
        "the stats and row queries should both pause: {checks:?}"
    );
    for (kind, waited, resumed) in &checks {
        assert!(
            resumed,
            "a cache lock was held through a page query's {kind}: it took {waited:?} to lock, \
             and the query gave up waiting first; all pauses: {checks:?}"
        );
    }
}
