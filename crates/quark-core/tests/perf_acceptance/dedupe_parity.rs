//! At 2M rows the rowid fast path and the row-numbering path keep the same rows in the same order.
// The dataset route dedupes a cache-backed source by its rowid; any other SQL numbers its rows.
// Both must agree on every page at a size DuckDB scans over all its threads.
// clippy.toml allows unwrap only inside `#[test]` fns; the helpers below are test code too.

use super::dedupe::sources;
use super::*;

/// One page of `target` on the narrow source's node, as its total and JSON rows.
fn page(target: &Target, body: &Value) -> (Value, Vec<Value>) {
    let state = &sources().state;
    let engine = state.engine_for_node(sources().narrow.node).unwrap();
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
    for (case, controls, pages) in cases {
        for number in pages {
            let mut body = controls.clone();
            body["page"] = number.into();
            body["page_size"] = 100.into();
            let expected = page(&by_rowid, &body);
            assert!(!expected.1.is_empty(), "{case}, page {number}");
            assert_eq!(page(&numbered, &body), expected, "{case}, page {number}");
        }
    }
}
