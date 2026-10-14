//! Performance acceptance on a 2M-row CSV (design section 8.6) and the spec section 9 route caps.
//!
//! Ignored because timings depend on the machine. Run on the development Mac with:
//! `cargo test -p quark-core --release --test perf_acceptance -- --ignored --nocapture`
//! The 2M-row setup and the timing helpers live here; `routes.rs` holds the spec section 9 cases.
//! Every case takes `SERIAL` first, so the cases never time each other. The route cases drive the
//! router over one shared 2M-row CSV and assert the median of a few runs against the confirmed cap.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.
#![allow(clippy::unwrap_used)]

mod routes;

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use arrow::ipc::reader::StreamReader;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use duckdb::Connection;
use http_body_util::BodyExt;
use quark_core::features::pages::model::{Format, PageBody};
use quark_core::features::pages::service::run_page;
use quark_core::features::sources::service::datasets;
use quark_core::features::sources::uploads::register_upload;
use quark_core::relation::request::QueryRequest;
use quark_core::relation::target::Target;
use quark_core::runtime::AppState;
use quark_core::runtime::dirs::Dirs;
use rust_xlsxwriter::Workbook;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower::ServiceExt;

const ROWS: u64 = 2_000_000;
const NODE: &str = "perf";
const WAIT: Duration = Duration::from_secs(120);
const RUNS: usize = 5;
const ARROW: &str = "application/vnd.apache.arrow.stream";
const WORKBOOK_ROWS: u32 = 100_000;
const WORKBOOK_COLUMNS: u16 = 10;

static SERIAL: Mutex<()> = Mutex::const_new(());

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
    let target = Target::Dataset(dataset.to_owned());
    let body = engine
        .read(None, |conn| {
            run_page(&engine, conn, &target, request, Format::Arrow)
        })
        .unwrap()
        .unwrap();
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
    let primary = engine.primary();
    let tables: i64 = primary
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
    let _serial = SERIAL.blocking_lock();
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
    register_upload(&state, None, NODE, "perf.csv", &csv).unwrap();
    let dataset = datasets(&state, NODE).unwrap().remove(0).info.id;
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

/// The shared 2M-row CSV, registered as node `perf` with its columnar import finished.
struct Perf {
    app: Router,
    state: AppState,
    csv: PathBuf,
    dataset: String,
    table: String,
}

impl Perf {
    fn base(&self) -> String {
        format!("/api/nodes/{NODE}/datasets/{}", self.dataset)
    }

    fn select_all(&self) -> String {
        format!("SELECT * FROM {}", self.table)
    }

    /// An export body over the first `limit` rows of the CSV.
    fn export_body(&self, format: &str, limit: u64) -> Value {
        let sql = format!("{} LIMIT {limit}", self.select_all());
        let sheet = json!({"node_id": NODE, "name": "All", "sql": sql});
        json!({"format": format, "filename": "perf", "sheets": [sheet]})
    }
}

fn perf() -> &'static Perf {
    static PERF: OnceLock<Perf> = OnceLock::new();
    PERF.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("perf-acceptance");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let csv = root.join("perf.csv");
        write_csv(&csv);
        let dirs = Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        };
        let state = AppState::load(dirs).unwrap();
        register_upload(&state, None, NODE, "perf.csv", &csv).unwrap();
        wait_until("the columnar import", || state.catalog().engines.is_empty());
        let info = datasets(&state, NODE).unwrap().remove(0).info;
        Perf {
            app: quark_core::router(state.clone()),
            state,
            csv,
            dataset: info.id,
            table: info.name,
        }
    })
}

/// Sends one request, reads the whole answer and returns the time that took.
async fn timed(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    accept: &str,
) -> Duration {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, accept);
    let body = body.map_or_else(Body::empty, |json| Body::from(json.to_string()));
    let started = Instant::now();
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let elapsed = started.elapsed();
    assert_eq!(
        status,
        200,
        "{method} {uri}: {}",
        String::from_utf8_lossy(&bytes)
    );
    elapsed
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// Prints the median and fails the case when it is over the cap.
fn assert_cap(case: &str, samples: Vec<Duration>, cap: Duration) {
    let observed = median(samples);
    println!(
        "{case:<40} {:>10.1} ms  (cap {} ms)",
        observed.as_secs_f64() * 1000.0,
        cap.as_millis()
    );
    assert!(
        observed <= cap,
        "{case}: median {observed:?} is over the {cap:?} cap"
    );
}

const fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// Times `RUNS` JSON requests built by `request(run)`.
async fn json_case(
    perf: &Perf,
    case: &str,
    cap: Duration,
    request: impl Fn(usize) -> (&'static str, String, Option<Value>),
) {
    let mut samples = Vec::with_capacity(RUNS);
    for run in 0..RUNS {
        let (method, uri, body) = request(run);
        samples.push(timed(&perf.app, method, &uri, body, "application/json").await);
    }
    assert_cap(case, samples, cap);
}

async fn chart_case(perf: &Perf, case: &str, cap: Duration, spec: Value) {
    let uri = format!("{}/visualize", perf.base());
    json_case(perf, case, cap, |_| {
        ("POST", uri.clone(), Some(json!({"spec": spec})))
    })
    .await;
}
