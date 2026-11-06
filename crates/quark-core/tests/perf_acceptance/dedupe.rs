//! A deduped View costs the same at any width: 52 columns within 1.5x of 8.
// 2M rows deduped and sorted by a number, as an 8-column source and as a 52-column one
// (30 VARCHAR), both read from the columnar cache. Two routes are timed: the dataset route that
// builder stats and charts use, and `/sql`, which the UI loads every page through with the
// View's plain-scan SQL. The 52-column count and first page may take at most 1.5x the 8-column
// ones, for a 50-value key and for a unique one; a selective filter must prune the scan.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.

use super::routes::reads_cache;
use super::*;
use duckdb::types::Value as Cell;
use quark_core::features::sources::service::project_views;
use quark_core::relation::filtered::filtered;
use quark_core::runtime::registry::DEFAULT_PROJECT_ID;
use quark_core::shared::sql::quote_ident;

const MAX_RATIO: f64 = 1.5;
const COUNT_CAP_MS: f64 = 100.0;
const PAGE_CAP_MS: f64 = 300.0;
const SELECTIVE_CAP_MS: f64 = 50.0;
const ROWS_KEPT: i64 = 2_000_000;

/// 2M rows of `id`, a 50-value `grp`, a numeric `price` and 5 more columns, 3 of the 8 VARCHAR.
/// The wide source adds 17 numbers and 27 texts: 52 columns, 30 of them VARCHAR.
fn write_source(path: &Path, is_wide: bool) {
    let mut columns: Vec<String> = [
        "i AS id",
        "(hash(i) % 50)::INTEGER AS grp",
        "round((hash(i + 1) % 1000000) / 100.0, 2) AS price",
        "(hash(i + 2) % 100)::INTEGER AS qty",
        "DATE '2020-01-01' + (hash(i + 3) % 1500)::INTEGER AS ordered",
        "'SKU-' || lpad(i::VARCHAR, 9, '0') AS sku",
        "'region-' || (hash(i + 4) % 8)::VARCHAR AS region",
        "'note ' || md5(i::VARCHAR) AS note",
    ]
    .map(str::to_owned)
    .to_vec();
    if is_wide {
        columns
            .extend((0..17).map(|k| format!("(hash(i + {}) % 100000)::BIGINT AS n{k}", 100 + k)));
        columns.extend((0..27).map(|k| {
            format!(
                "'text {k} ' || (hash(i + {}) % 100000)::VARCHAR AS t{k}",
                200 + k
            )
        }));
    }
    let target = path.to_string_lossy().replace('\'', "''");
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "COPY (SELECT {} FROM range({ROWS}) t(i)) TO '{target}' (FORMAT parquet)",
        columns.join(", ")
    ))
    .unwrap();
}

/// One uploaded source: its node and dataset, and the project View the UI opens over it.
pub(super) struct Source {
    pub(super) node: &'static str,
    pub(super) dataset: String,
    /// The dataset's view on the node engine, quoted.
    pub(super) table: String,
    view_node: String,
    view_sql: String,
}

pub(super) struct Sources {
    pub(super) state: AppState,
    app: Router,
    pub(super) narrow: Source,
    wide: Source,
}

/// The 8- and 52-column sources, uploaded once and read from their columnar cache.
pub(super) fn sources() -> &'static Sources {
    static SOURCES: OnceLock<Sources> = OnceLock::new();
    SOURCES.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dedupe-acceptance");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dirs = Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        };
        let state = AppState::load(dirs).unwrap();
        let nodes = [("dedupe-narrow", false), ("dedupe-wide", true)].map(|(node, is_wide)| {
            let name = format!("{node}.parquet");
            let file = root.join(&name);
            write_source(&file, is_wide);
            register_upload(&state, None, node, &name, &file).unwrap();
            node
        });
        let views = project_views(&state, DEFAULT_PROJECT_ID).unwrap();
        let [narrow, wide] = nodes.map(|node| {
            let info = datasets(&state, node).unwrap().remove(0).info;
            let view = views.iter().find(|view| view.source_id == node).unwrap();
            // No case may time a live file scan: the node and the project View read the cache.
            wait_until("the columnar import", || reads_cache(&state, node));
            wait_until("the View's columnar import", || {
                reads_cache(&state, &view.node_id)
            });
            Source {
                node,
                dataset: info.id,
                table: format!("{}.{}", quote_ident(&info.schema), quote_ident(&info.name)),
                view_node: view.node_id.clone(),
                view_sql: view.sql.clone(),
            }
        });
        Sources {
            app: quark_core::router(state.clone()),
            state,
            narrow,
            wide,
        }
    })
}

/// A first-page request sorted by price and deduped on `key`, with `filters`.
fn deduped(key: &str, filters: Value) -> Value {
    json!({
        "page": 1,
        "page_size": 100,
        "filters": filters,
        "sorts": [{"column": "price", "direction": "desc"}],
        "dedupe_columns": [key],
    })
}

/// A filter on `id` above `floor` plus a number no earlier request used, so no cache answers it.
fn above(floor: i64) -> Value {
    let fresh = FRESH.fetch_add(1, Ordering::Relaxed);
    json!([{"column": "id", "operator": ">", "value": floor + fresh}])
}

fn cell(value: &Value) -> Cell {
    match value {
        Value::String(text) => Cell::Text(text.clone()),
        other => Cell::BigInt(other.as_i64().unwrap()),
    }
}

/// Times the deduped row count of `target` the way stats, charts and the page total read it.
fn timed_count(node: &str, target: &Target, body: &Value) -> (Duration, i64) {
    let engine = sources().state.engine_for_node(node).unwrap();
    let request: QueryRequest = serde_json::from_value(body.clone()).unwrap();
    let started = Instant::now();
    let count = engine
        .read(None, |conn| {
            let relation = filtered(conn, target, &request).unwrap();
            conn.query_row(
                &format!("SELECT count(*) FROM {}", relation.sql),
                duckdb::params_from_iter(relation.params.iter().map(cell)),
                |row| row.get(0),
            )
            .unwrap()
        })
        .unwrap();
    (started.elapsed(), count)
}

/// Times one first page of the dataset route: count, null fractions and Arrow rows.
fn timed_dataset_page(source: &Source, body: &Value) -> Duration {
    let engine = sources().state.engine_for_node(source.node).unwrap();
    let target = Target::Dataset(source.dataset.clone());
    let request: QueryRequest = serde_json::from_value(body.clone()).unwrap();
    let started = Instant::now();
    let page = engine
        .read(None, |conn| {
            run_page(&engine, conn, &target, &request, Format::Arrow)
        })
        .unwrap()
        .unwrap();
    let elapsed = started.elapsed();
    let PageBody::Arrow(bytes) = page else {
        panic!("expected an Arrow page");
    };
    let rows: usize = StreamReader::try_new(Cursor::new(bytes), None)
        .unwrap()
        .map(|batch| batch.unwrap().num_rows())
        .sum();
    assert_eq!(rows, 50, "{}", source.node);
    elapsed
}

/// The median of `RUNS` timings after one untimed warm-up.
fn median_of(mut run: impl FnMut() -> Duration) -> Duration {
    run();
    median((0..RUNS).map(|_| run()).collect())
}

/// The 52-column time may be at most 1.5x the 8-column one: a target like the caps.
fn within(narrow: Duration) -> Option<f64> {
    Some(narrow.as_secs_f64() * 1000.0 * MAX_RATIO)
}

#[test]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
fn deduped_view_costs_the_same_at_8_and_52_columns() {
    let _serial = SERIAL.blocking_lock();
    let sources = sources();
    let mut table = Table::default();
    let mut measured = Vec::new();
    for source in [&sources.narrow, &sources.wide] {
        let target = Target::Dataset(source.dataset.clone());
        let count = median_of(|| {
            let (elapsed, count) = timed_count(source.node, &target, &deduped("grp", above(0)));
            assert_eq!(count, 50, "{}", source.node);
            elapsed
        });
        let page = median_of(|| timed_dataset_page(source, &deduped("grp", above(0))));
        measured.push((count, page));
    }
    let [(narrow_count, narrow_page), (wide_count, wide_page)] = measured.try_into().unwrap();
    table.add("deduped count, 8 columns", narrow_count, Some(COUNT_CAP_MS));
    table.add(
        "deduped count, 52 columns (30 VARCHAR)",
        wide_count,
        Some(COUNT_CAP_MS),
    );
    table.add(
        "deduped count, 52 columns, within 1.5x of 8",
        wide_count,
        within(narrow_count),
    );
    table.add(
        "deduped first page, 8 columns",
        narrow_page,
        Some(PAGE_CAP_MS),
    );
    table.add(
        "deduped first page, 52 columns (30 VARCHAR)",
        wide_page,
        Some(PAGE_CAP_MS),
    );
    table.add(
        "deduped first page, 52 columns, within 1.5x of 8",
        wide_page,
        within(narrow_page),
    );
    table.finish();
}

/// Times one first page as the UI loads it: `POST /sql` with the View's SQL, read as Arrow.
fn timed_sql_page(runtime: &tokio::runtime::Runtime, source: &Source, body: &Value) -> Duration {
    let mut body = body.clone();
    body["sql"] = source.view_sql.clone().into();
    let uri = format!("/api/nodes/{}/sql", source.view_node);
    runtime.block_on(timed(&sources().app, "POST", &uri, Some(body), ARROW))
}

#[test]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
fn deduped_sql_route_costs_the_same_at_8_and_52_columns() {
    let _serial = SERIAL.blocking_lock();
    let sources = sources();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut table = Table::default();
    // A 50-value key keeps 50 rows; a unique key keeps all of them, so its page sorts 2M rows.
    for (key, kept) in [("grp", 50..=50), ("sku", 1_990_000..=ROWS_KEPT)] {
        let mut measured = Vec::new();
        for source in [&sources.narrow, &sources.wide] {
            let target = Target::Sql(source.view_sql.clone());
            let count = median_of(|| {
                let (elapsed, count) =
                    timed_count(&source.view_node, &target, &deduped(key, above(0)));
                assert!(kept.contains(&count), "{key}: {count} rows");
                elapsed
            });
            let page = median_of(|| timed_sql_page(&runtime, source, &deduped(key, above(0))));
            measured.push((count, page));
        }
        let [(narrow_count, narrow_page), (wide_count, wide_page)] = measured.try_into().unwrap();
        let step = |what: &str| format!("/sql, dedupe on {key}: {what}");
        table.add(&step("count, 8 columns"), narrow_count, Some(COUNT_CAP_MS));
        table.add(&step("count, 52 columns"), wide_count, Some(COUNT_CAP_MS));
        table.add(
            &step("count, within 1.5x"),
            wide_count,
            within(narrow_count),
        );
        table.add(
            &step("first page, 8 columns"),
            narrow_page,
            Some(PAGE_CAP_MS),
        );
        table.add(
            &step("first page, 52 columns"),
            wide_page,
            Some(PAGE_CAP_MS),
        );
        table.add(
            &step("first page, within 1.5x"),
            wide_page,
            within(narrow_page),
        );
    }
    // About 1,000 rows pass `id > 1999000`: numbered after the filter, the scan is pruned.
    let wide = &sources.wide;
    let target = Target::Sql(wide.view_sql.clone());
    let count = median_of(|| {
        let (elapsed, count) =
            timed_count(&wide.view_node, &target, &deduped("grp", above(1_999_000)));
        assert!((1..=50).contains(&count), "{count} rows");
        elapsed
    });
    let page = median_of(|| timed_sql_page(&runtime, wide, &deduped("grp", above(1_999_000))));
    table.add(
        "/sql, id > 1999000, dedupe on grp: count, 52 columns",
        count,
        Some(SELECTIVE_CAP_MS),
    );
    table.add(
        "/sql, id > 1999000, dedupe on grp: first page, 52 columns",
        page,
        Some(SELECTIVE_CAP_MS),
    );
    table.finish();
}
