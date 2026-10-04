//! Performance acceptance on a 2M-row CSV (design section 8.6), through `AppState`.
//!
//! Ignored because timings depend on the machine. Run on the development Mac with:
//! `cargo test -p quark-core --release --test perf_acceptance -- --ignored --nocapture`
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.
#![allow(clippy::unwrap_used)]

use std::io::Cursor;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use arrow::ipc::reader::StreamReader;
use duckdb::Connection;
use quark_core::engine::Dirs;
use quark_core::page::{Format, PageBody, PageTarget, run_page};
use quark_core::query::QueryRequest;
use quark_core::state::AppState;
use serde_json::{Value, json};

const ROWS: u64 = 2_000_000;
const NODE: &str = "perf";
const WAIT: Duration = Duration::from_secs(120);

/// 2M rows by 10 columns, about 250 MB, with the `qty` and `price` columns the checks use.
fn write_csv(path: &Path) {
    let target = path.to_string_lossy().replace('\'', "''");
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "COPY (SELECT
            i AS id,
            'SKU-' || lpad(i::VARCHAR, 9, '0') AS sku,
            'Item ' || (hash(i) % 100000)::VARCHAR AS name,
            'category-' || (hash(i + 1) % 40)::VARCHAR AS category,
            'region-' || (hash(i + 2) % 8)::VARCHAR AS region,
            (hash(i + 3) % 100)::INTEGER AS qty,
            round((hash(i + 4) % 1000000) / 100.0, 2) AS price,
            round((hash(i + 5) % 3000) / 100.0, 2) AS discount,
            DATE '2020-01-01' + (hash(i + 6) % 1500)::INTEGER AS ordered,
            CASE WHEN hash(i + 7) % 10 = 0 THEN NULL
                 ELSE 'note ' || md5(i::VARCHAR) || ' for the order' END AS note
         FROM range({ROWS}) t(i)) TO '{target}' (HEADER)"
    ))
    .unwrap();
}

/// One 100-row page request; `controls` holds the filters or sorts.
fn request(page: i64, controls: Value) -> QueryRequest {
    let mut body = controls;
    body["page"] = page.into();
    serde_json::from_value(body).unwrap()
}

fn filtered(page: i64) -> QueryRequest {
    let filters = json!([{"column": "qty", "operator": ">", "value": 10}]);
    request(page, json!({ "filters": filters }))
}

fn sorted_by_price(page: i64) -> QueryRequest {
    request(
        page,
        json!({ "sorts": [{"column": "price", "direction": "asc"}] }),
    )
}

/// Times one page as the HTTP route does, from engine lookup to Arrow bytes, and checks it is full.
fn timed_page(state: &AppState, dataset: &str, request: &QueryRequest) -> Duration {
    let started = Instant::now();
    let engine = state.engine_for_node(NODE).unwrap();
    let target = PageTarget::Dataset(dataset.to_owned());
    let body = run_page(&mut engine.lock(), &target, request, Format::Arrow).unwrap();
    let elapsed = started.elapsed();
    let PageBody::Arrow(bytes) = body else {
        panic!("expected an Arrow page");
    };
    let rows: usize = StreamReader::try_new(Cursor::new(bytes), None)
        .unwrap()
        .map(|batch| batch.unwrap().num_rows())
        .sum();
    assert_eq!(rows, 100, "page {}", request.page);
    elapsed
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn has_result_table(state: &AppState) -> bool {
    let engine = state.engine_for_node(NODE).unwrap();
    let inner = engine.lock();
    let tables: i64 = inner
        .conn
        .query_row(
            "SELECT count(*) FROM duckdb_tables() WHERE schema_name = 'quark_results'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    tables > 0
}

/// Steps with their time and target in milliseconds; `None` means the step is only recorded.
#[derive(Default)]
struct Table(Vec<(String, Duration, Option<f64>)>);

impl Table {
    fn add(&mut self, step: &str, elapsed: Duration, limit_ms: Option<f64>) {
        self.0.push((step.to_owned(), elapsed, limit_ms));
    }

    /// Prints every step, then fails if any target was missed.
    fn finish(&self) {
        println!("\n{:<58} {:>9}  target", "step", "ms");
        let mut missed = Vec::new();
        for (step, elapsed, limit_ms) in &self.0 {
            let ms = elapsed.as_secs_f64() * 1000.0;
            let verdict = match limit_ms {
                None => "recorded".to_owned(),
                Some(limit) if ms <= *limit => format!("<= {limit} ms ok"),
                Some(limit) => {
                    missed.push(step.as_str());
                    format!("<= {limit} ms MISSED")
                }
            };
            println!("{step:<58} {ms:>9.2}  {verdict}");
        }
        assert!(missed.is_empty(), "targets missed: {missed:?}");
    }
}

#[test]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
fn two_million_row_csv_meets_the_targets() {
    let root = tempfile::tempdir().unwrap();
    let csv = root.path().join("perf.csv");
    write_csv(&csv);
    println!(
        "generated {ROWS} rows, {} MB",
        std::fs::metadata(&csv).unwrap().len() / 1_000_000
    );

    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let state = AppState::load(dirs).unwrap();
    state.register_upload(None, NODE, "perf.csv", &csv).unwrap();
    let dataset = state.datasets(NODE).unwrap().remove(0).info.id;
    let mut table = Table::default();

    // The upload queued the columnar import, so this page runs on the live CSV beside it.
    let first = timed_page(&state, &dataset, &filtered(1));
    table.add(
        "first page, qty > 10 (live CSV, import running)",
        first,
        Some(750.0),
    );
    let import = Instant::now();
    wait_until("the columnar import", || state.catalog().engines.is_empty());
    table.add(
        "columnar import finished (background)",
        import.elapsed(),
        None,
    );
    let deep = timed_page(&state, &dataset, &filtered(10_001));
    table.add(
        "after the import: page at offset 1,000,000, qty > 10",
        deep,
        Some(60.0),
    );

    // The second identical request starts the result build; the table then serves every page.
    for step in ["first request", "second request (starts the result build)"] {
        let elapsed = timed_page(&state, &dataset, &sorted_by_price(1));
        table.add(&format!("sort by price: {step}"), elapsed, None);
    }
    let build = Instant::now();
    wait_until("the sorted result", || has_result_table(&state));
    // The build marks itself Ready right after its table lands; give that a moment.
    thread::sleep(Duration::from_millis(50));
    table.add("sorted result ready (background)", build.elapsed(), None);
    for (page, offset) in [(1, 0), (5_001, 500_000), (10_001, 1_000_000)] {
        let elapsed = timed_page(&state, &dataset, &sorted_by_price(page));
        table.add(
            &format!("sorted by price: page at offset {offset}"),
            elapsed,
            Some(5.0),
        );
    }

    table.finish();
    state.shutdown();
}
