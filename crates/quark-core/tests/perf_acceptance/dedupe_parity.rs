//! At 2M rows the rowid fast path and the row-numbering path keep the same rows in the same order.
// The dataset route dedupes a cache-backed source by its rowid; any other SQL numbers its rows.
// Both must agree on every page at a size DuckDB scans over all its threads, for the plain View
// and for a View the user changed with a formula or a replaced column.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.

use super::dedupe::{Source, sources};
use super::*;

/// One page of `target` on `node`, as its total and JSON rows.
fn page(node: &str, target: &Target, body: &Value) -> (Value, Vec<Value>) {
    let state = &sources().state;
    let engine = state.engine_for_node(node).unwrap();
    let request: QueryRequest = serde_json::from_value(body.clone()).unwrap();
    let page = engine
        .read(None, |conn| {
            run_page(&engine, conn, target, &request, Format::Json)
        })
        .unwrap()
        .unwrap();
    let PageBody::Json(page) = page else {
        panic!("expected a JSON page");
    };
    (
        page["total_rows"].clone(),
        page["rows"].as_array().unwrap().clone(),
    )
}

/// Checks that each case's pages from `fast` on `node` equal those from `numbered`.
fn assert_same_pages(
    node: &str,
    fast: &Target,
    numbered: &Target,
    cases: &[(&str, Value, Vec<i64>)],
) {
    for (case, controls, pages) in cases {
        for &number in pages {
            let mut body = controls.clone();
            body["page"] = number.into();
            body["page_size"] = 100.into();
            let expected = page(node, fast, &body);
            assert!(!expected.1.is_empty(), "{case}, page {number}");
            assert_eq!(
                page(node, numbered, &body),
                expected,
                "{case}, page {number}"
            );
        }
    }
}

#[test]
#[ignore = "needs the 2M-row sources; run with --ignored in a release build"]
fn rowid_and_row_numbering_keep_the_same_rows_in_the_same_order() {
    let _serial = SERIAL.blocking_lock();
    let source = &sources().narrow;
    let by_rowid = Target::Dataset(source.dataset.clone());
    // `WHERE true` keeps every row but is no plain scan of the View, so its rows are numbered.
    let numbered = Target::Sql(format!("SELECT * FROM {} WHERE true", source.table));
    let price = json!([{"column": "price", "direction": "desc"}]);
    let qty = json!([{"column": "qty", "direction": "asc"}]);
    let cases = [
        (
            "50-value key, sorted",
            json!({"sorts": price, "dedupe_columns": ["grp"]}),
            vec![1],
        ),
        (
            "100-value key, no sort",
            json!({"dedupe_columns": ["qty"]}),
            vec![1],
        ),
        // Each qty ties about 20,000 rows, so position orders nearly every row.
        (
            "unique key, tied sort",
            json!({"sorts": qty, "dedupe_columns": ["sku"]}),
            vec![1, 10_000, 20_000],
        ),
    ];
    assert_same_pages(source.node, &by_rowid, &numbered, &cases);
}

/// The View `change` writes over `source`, with the View's own SQL first as its source and then
/// a no-op LIMIT over it, which is no projection of the cached scan, so its rows are numbered.
fn fast_and_numbered(source: &Source, change: impl Fn(&Source) -> String) -> (Target, Target) {
    let fast = change(source);
    let view_sql = format!(
        "SELECT * FROM ({}) LIMIT 9223372036854775807",
        source.view_sql
    );
    let numbered = fast.replace(
        &format!("(\n{}\n)", source.view_sql),
        &format!("(\n{view_sql}\n)"),
    );
    assert_ne!(fast, numbered);
    (Target::Sql(fast), Target::Sql(numbered))
}

#[test]
#[ignore = "needs the 2M-row sources; run with --ignored in a release build"]
fn changed_views_keep_the_same_rows_on_the_fast_and_the_numbering_path() {
    let _serial = SERIAL.blocking_lock();
    let source = &sources().narrow;
    let price = json!([{"column": "price", "direction": "desc"}]);
    let p2 = json!([{"column": "p2", "direction": "desc"}]);
    let qty = json!([{"column": "qty", "direction": "asc"}]);
    let (fast, numbered) = fast_and_numbered(source, Source::with_formula);
    let cases = [
        (
            "formula, dedupe on grp",
            json!({"sorts": price, "dedupe_columns": ["grp"]}),
            vec![1],
        ),
        // Each qty ties about 20,000 rows, so position orders nearly every row.
        (
            "formula, unique key, tied sort",
            json!({"sorts": qty, "dedupe_columns": ["sku"]}),
            vec![1, 20_000],
        ),
    ];
    assert_same_pages(&source.view_node, &fast, &numbered, &cases);
    let (fast, numbered) = fast_and_numbered(source, Source::stacked);
    let cases = [
        (
            "formula + replaced column, by p2",
            json!({"sorts": p2, "dedupe_columns": ["grp"]}),
            vec![1],
        ),
        (
            "formula + replaced column, unique key, tied sort",
            json!({"sorts": qty, "dedupe_columns": ["sku"]}),
            vec![1, 20_000],
        ),
    ];
    assert_same_pages(&source.view_node, &fast, &numbered, &cases);
}
