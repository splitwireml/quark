//! A deduped View costs the same at any width: 52 columns within 1.5x of 8.
// 2M rows deduped on a 50-value key and sorted by a number, as an 8-column source and as a
// 52-column one (30 VARCHAR), both read from the columnar cache. The 52-column count and first
// page may take at most 1.5x the 8-column ones.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.

use super::routes::reads_cache;
use super::*;
use quark_core::relation::filtered::filtered;

const NARROW: &str = "dedupe-narrow";
const WIDE: &str = "dedupe-wide";
const MAX_RATIO: f64 = 1.5;
const COUNT_CAP_MS: f64 = 100.0;
const PAGE_CAP_MS: f64 = 300.0;

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

/// The deduped, sorted View, with a filter no earlier request has sent so no cache answers it.
fn deduped() -> QueryRequest {
    request(
        1,
        json!({
            "filters": fresh_filter(),
            "sorts": [{"column": "price", "direction": "desc"}],
            "dedupe_columns": ["grp"],
        }),
    )
}

/// Times the deduped row count the way stats and charts read the View.
fn timed_count(state: &AppState, node: &str, dataset: &str) -> Duration {
    let engine = state.engine_for_node(node).unwrap();
    let target = Target::Dataset(dataset.to_owned());
    let request = deduped();
    let started = Instant::now();
    let count: i64 = engine
        .read(None, |conn| {
            let relation = filtered(conn, &target, &request).unwrap();
            let params = relation.params.iter().map(|value| value.as_i64().unwrap());
            conn.query_row(
                &format!("SELECT count(*) FROM {}", relation.sql),
                duckdb::params_from_iter(params),
                |row| row.get(0),
            )
            .unwrap()
        })
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(count, 50, "{node}");
    elapsed
}

/// Times the first page as the HTTP route serves it: count, null fractions and 50 Arrow rows.
fn timed_first_page(state: &AppState, node: &str, dataset: &str) -> Duration {
    let engine = state.engine_for_node(node).unwrap();
    let target = Target::Dataset(dataset.to_owned());
    let request = deduped();
    let started = Instant::now();
    let body = engine
        .read(None, |conn| {
            run_page(&engine, conn, &target, &request, Format::Arrow)
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
    assert_eq!(rows, 50, "{node}");
    elapsed
}

/// Median count and first-page times over `RUNS` fresh requests, after one untimed warm-up.
fn measure(state: &AppState, node: &str, dataset: &str) -> (Duration, Duration) {
    timed_first_page(state, node, dataset);
    let counts = (0..RUNS)
        .map(|_| timed_count(state, node, dataset))
        .collect();
    let pages = (0..RUNS)
        .map(|_| timed_first_page(state, node, dataset))
        .collect();
    (median(counts), median(pages))
}

#[test]
#[ignore = "timings depend on the machine; run with --ignored in a release build"]
fn deduped_view_costs_the_same_at_8_and_52_columns() {
    let _serial = SERIAL.blocking_lock();
    let root = tempfile::tempdir().unwrap();
    let dirs = Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let state = AppState::load(dirs).unwrap();
    let mut datasets_by_node = Vec::new();
    for (node, is_wide) in [(NARROW, false), (WIDE, true)] {
        let name = format!("{node}.parquet");
        let file = root.path().join(&name);
        write_source(&file, is_wide);
        register_upload(&state, None, node, &name, &file).unwrap();
        datasets_by_node.push((node, datasets(&state, node).unwrap().remove(0).info.id));
    }
    // Both sources must read their columnar cache, so neither case times a live file scan.
    for (node, _) in &datasets_by_node {
        wait_until("the columnar import", || reads_cache(&state, node));
    }

    let [(narrow, narrow_dataset), (wide, wide_dataset)] = datasets_by_node.try_into().unwrap();
    let (narrow_count, narrow_page) = measure(&state, narrow, &narrow_dataset);
    let (wide_count, wide_page) = measure(&state, wide, &wide_dataset);

    // Width independence is a target like the caps: the 52-column time within 1.5x of the 8-column one.
    let within = |narrow: Duration| Some(narrow.as_secs_f64() * 1000.0 * MAX_RATIO);
    let mut table = Table::default();
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
    state.shutdown();
}
