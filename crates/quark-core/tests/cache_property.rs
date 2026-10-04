//! Caches may make a page faster but never different: every page, at every stage of a query's
//! life, equals what a plain DuckDB connection over the CSV returns for the same `build_query`.
// The helpers below are test code too, which clippy.toml's allowance does not reach in this crate.
#![allow(clippy::unwrap_used)]

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Days, NaiveDate};
use duckdb::Connection;
use duckdb::params_from_iter;
use duckdb::types::Value as Cell;
use quark_core::cache::results::ResultAction;
use quark_core::cache::stats::stats_key;
use quark_core::engine::{Dirs, Engine, describe};
use quark_core::ids::dataset_id;
use quark_core::page::{Format, PageBody, PageTarget, run_page};
use quark_core::query::{
    ColumnMeta, Connector, Direction, Filter, QueryRequest, Sort, build_query,
};
use quark_core::registry::SourceRecord;
use quark_core::sql::scan_expression;
use quark_core::state::AppState;
use serde_json::{Value, json};

const ROWS: i64 = 20_000;
const PAGE_SIZE: i64 = 100;
const TABLE: &str = r#""main"."data""#;
const WAIT: Duration = Duration::from_secs(30);
const CATEGORIES: [&str; 5] = ["alpha", "beta", "gamma", "delta", "epsilon"];

/// 20,000 rows: a unique `id`, integers with every seventh null, text categories and dates.
fn write_csv(path: &Path) {
    let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let mut text = String::from("id,n,cat,d\n");
    for id in 0..ROWS {
        let number = if id % 7 == 0 {
            String::new()
        } else {
            ((id * 37) % 1000).to_string()
        };
        let category = CATEGORIES[usize::try_from(id / 3 % 5).unwrap()];
        let date = start + Days::new(u64::try_from(id * 13 % 1500).unwrap());
        writeln!(text, "{id},{number},{category},{date}").unwrap();
    }
    fs::write(path, text).unwrap();
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn filter(column: &str, operator: &str, value: Value, connector: Connector) -> Filter {
    Filter {
        column: column.to_owned(),
        operator: operator.to_owned(),
        value: Some(value),
        connector,
    }
}

fn sort(column: &str, direction: Direction) -> Sort {
    Sort {
        column: column.to_owned(),
        direction,
    }
}

struct Case {
    name: String,
    filters: Vec<Filter>,
    sorts: Vec<Sort>,
    dedupe: Vec<String>,
}

impl Case {
    fn request(&self, page: i64) -> QueryRequest {
        QueryRequest {
            page,
            page_size: PAGE_SIZE,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe.clone(),
        }
    }

    /// Filters, sorts or dedupe: a plain scan never gets a result table.
    fn has_controls(&self) -> bool {
        !(self.filters.is_empty() && self.sorts.is_empty() && self.dedupe.is_empty())
    }
}

/// 8 filter sets (0 to 3 filters, with `or`, `in`, `between` and `contains`) times 4 sort and
/// dedupe shapes (every sort list ends with `id`), so 32 cases.
fn cases() -> Vec<Case> {
    use Connector::{And, Or};
    let any_of = |connector| filter("cat", "in", json!(["alpha", "gamma"]), connector);
    let between = |connector| filter("n", "between", json!([100, 600]), connector);
    let contains = |connector| filter("cat", "contains", json!("ta"), connector);
    let since = |connector| filter("d", ">=", json!("2022-01-01"), connector);
    let filter_sets = [
        ("none", vec![]),
        ("in", vec![any_of(And)]),
        ("between", vec![between(And)]),
        ("contains", vec![contains(And)]),
        ("in and between", vec![any_of(And), between(And)]),
        ("in or between", vec![any_of(And), between(Or)]),
        (
            "contains and between or since",
            vec![contains(And), between(And), since(Or)],
        ),
        (
            "between and since or contains",
            vec![between(And), since(And), contains(Or)],
        ),
    ];
    let shapes = [
        ("unsorted", vec![], vec![]),
        ("by id", vec![sort("id", Direction::Asc)], vec![]),
        (
            "by n desc then id",
            vec![sort("n", Direction::Desc), sort("id", Direction::Asc)],
            vec![],
        ),
        (
            "deduped on n, by n then id",
            vec![sort("n", Direction::Desc), sort("id", Direction::Asc)],
            vec!["n".to_owned()],
        ),
    ];
    let mut cases = Vec::new();
    for (filter_name, filters) in &filter_sets {
        for (shape_name, sorts, dedupe) in &shapes {
            cases.push(Case {
                name: format!("{filter_name}, {shape_name}"),
                filters: filters.clone(),
                sorts: sorts.clone(),
                dedupe: dedupe.clone(),
            });
        }
    }
    cases
}

fn cell_json(cell: Cell) -> Value {
    match cell {
        Cell::Null => Value::Null,
        Cell::BigInt(number) => json!(number),
        Cell::Text(text) => json!(text),
        Cell::Date32(days) => {
            let date = DateTime::from_timestamp(i64::from(days) * 86_400, 0).unwrap();
            json!(date.date_naive().to_string())
        }
        other => panic!("unexpected cell type: {other:?}"),
    }
}

fn cells(params: &[Value]) -> Vec<Cell> {
    params
        .iter()
        .map(|param| match param {
            Value::String(text) => Cell::Text(text.clone()),
            Value::Number(number) => Cell::BigInt(number.as_i64().unwrap()),
            other => panic!("unexpected parameter: {other}"),
        })
        .collect()
}

/// A plain DuckDB connection over the CSV: no engine, no caches.
struct Oracle {
    conn: Connection,
    columns: Vec<ColumnMeta>,
}

struct Expected {
    total_rows: u64,
    rows: Vec<Value>,
}

impl Oracle {
    fn open(csv: &Path) -> Self {
        let conn = Connection::open_in_memory().unwrap();
        let scan = scan_expression(&csv.to_string_lossy()).unwrap();
        conn.execute_batch(&format!("CREATE TABLE data AS SELECT * FROM {scan}"))
            .unwrap();
        let columns = describe(&conn, TABLE, &[]).unwrap();
        Self { conn, columns }
    }

    fn expect(&self, request: &QueryRequest) -> Expected {
        let built = build_query(TABLE, TABLE, &self.columns, request).unwrap();
        let total_rows = self
            .conn
            .query_row(
                &format!("SELECT count(*) FROM {}", built.relation),
                params_from_iter(cells(&built.params)),
                |row| row.get(0),
            )
            .unwrap();
        let mut params = cells(&built.params);
        params.push(Cell::BigInt(request.page_size));
        params.push(Cell::BigInt((request.page - 1) * request.page_size));
        let mut statement = self
            .conn
            .prepare(&format!(
                "SELECT * FROM ({}) AS result LIMIT ? OFFSET ?",
                built.ordered
            ))
            .unwrap();
        let rows = statement
            .query_map(params_from_iter(params), |row| {
                let object = self.columns.iter().enumerate().map(|(index, meta)| {
                    let cell = row.get::<_, Cell>(index).unwrap();
                    (meta.name.clone(), cell_json(cell))
                });
                Ok(Value::Object(object.collect()))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        Expected { total_rows, rows }
    }

    /// The key the result cache files this request under.
    fn result_key(&self, request: &QueryRequest) -> String {
        let built = build_query(TABLE, TABLE, &self.columns, request).unwrap();
        stats_key(&built.ordered, &built.params)
    }
}

/// DuckDB keeps an arbitrary row of each dedupe group, so a deduped page is compared on its key
/// columns only; every other page is compared whole.
fn comparable(case: &Case, rows: Vec<Value>) -> Vec<Value> {
    if case.dedupe.is_empty() {
        return rows;
    }
    let keys = |row: Value| {
        let kept = case
            .dedupe
            .iter()
            .map(|key| (key.clone(), row[key].clone()));
        Value::Object(kept.collect())
    };
    rows.into_iter().map(keys).collect()
}

/// The first page, a middle one, the last, and one past the end.
fn pages(oracle: &Oracle, case: &Case) -> Vec<i64> {
    let total = oracle.expect(&case.request(1)).total_rows;
    let last = i64::try_from(total.div_ceil(PAGE_SIZE.unsigned_abs()))
        .unwrap()
        .max(1);
    let mut pages = vec![1, (last + 1) / 2, last, last + 1];
    pages.dedup();
    pages
}

fn assert_matches(engine: &Engine, case: &Case, page: i64, expected: &Expected, phase: &str) {
    let request = case.request(page);
    let body = {
        let mut inner = engine.lock();
        let target = PageTarget::Dataset(dataset_id("main", "data"));
        run_page(&mut inner, &target, &request, Format::Json).unwrap()
    };
    let PageBody::Json(body) = body else {
        panic!("expected a JSON body");
    };
    let context = format!("{} / page {page} / {phase}", case.name);
    assert_eq!(body["total_rows"], expected.total_rows, "{context}");
    let rows = body["rows"].as_array().unwrap().clone();
    assert!(
        comparable(case, rows) == comparable(case, expected.rows.clone()),
        "{context}"
    );
}

/// The CSV and the data and cache folders of one test.
fn fixture(root: &Path) -> (PathBuf, Dirs) {
    let csv = root.join("rows.csv");
    write_csv(&csv);
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    (csv, dirs)
}

#[test]
fn first_second_and_built_results_match_the_oracle() {
    let root = tempfile::tempdir().unwrap();
    let (csv, dirs) = fixture(root.path());
    let source = SourceRecord {
        id: "legacy".to_owned(),
        name: "LEGACY".to_owned(),
        kind: "csv".to_owned(),
        source: csv.clone(),
        project_id: None,
        dataset_name: None,
        sheets: None,
    };
    let oracle = Oracle::open(&csv);

    // Reading the CSV is slow, so each case gets one engine. Its first and second requests rotate
    // through the offsets from case to case; once the result is built, every offset is checked.
    for (index, case) in cases().iter().enumerate() {
        let offsets = pages(&oracle, case);
        let expected: Vec<_> = offsets
            .iter()
            .map(|&page| oracle.expect(&case.request(page)))
            .collect();
        let engine = Engine::open_node(&source, &dirs, 1).unwrap();
        let check = |at: usize, phase| {
            assert_matches(&engine, case, offsets[at], &expected[at], phase);
        };
        check(index % offsets.len(), "first request");
        check((index + 1) % offsets.len(), "second request");
        if case.has_controls() {
            let key = oracle.result_key(&case.request(1));
            wait_until("the result build", || {
                let action = engine.lock().results.note_request(&key);
                matches!(action, ResultAction::ServeFrom(_))
            });
        }
        for at in 0..offsets.len() {
            check(at, "after the result build");
        }
    }
}

#[test]
fn columnar_switch_matches_the_oracle() {
    let root = tempfile::tempdir().unwrap();
    let (csv, dirs) = fixture(root.path());
    fs::create_dir_all(&dirs.data).unwrap();
    let registry = json!([{
        "id": "legacy", "name": "LEGACY", "kind": "csv", "source": csv.to_string_lossy(),
    }]);
    fs::write(dirs.registry_file(), registry.to_string()).unwrap();
    fs::write(dirs.projects_file(), "[]").unwrap();
    let state = AppState::load(dirs).unwrap();
    let oracle = Oracle::open(&csv);

    // The first request mounts the CSV and queues its import; the engine is replaced once it lands.
    let mut switched = None;
    wait_until("the columnar switch", || {
        let engine = state.engine_for_node("legacy").unwrap();
        let definition: String = engine
            .lock()
            .conn
            .query_row(
                "SELECT sql FROM duckdb_views() WHERE NOT internal AND view_name = 'data'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let is_columnar = definition.contains("cache_");
        if is_columnar {
            switched = Some(engine);
        }
        is_columnar
    });
    let engine = switched.unwrap();

    for case in cases() {
        for page in pages(&oracle, &case) {
            let expected = oracle.expect(&case.request(page));
            assert_matches(&engine, &case, page, &expected, "after the columnar switch");
        }
    }
    state.shutdown();
}
