//! Tests that the export readers take the result batch by batch and in order.

use std::sync::Arc;

use duckdb::Connection;
use duckdb::arrow::array::{ArrayRef, BooleanArray, Int64Array};
use duckdb::arrow::record_batch::RecordBatch;

use super::{data_rows, for_each_batch, stopped_early};
use crate::error::ApiResult;

const ROWS: i64 = 10_000;
/// Large enough that DuckDB's buffer fills and the producer is still running when batches are read.
const LONG_RUN: i64 = 3_000_000;
/// What DuckDB says for `error('late failure')`. Python duckdb 1.5.4 gives the same `str(error)`
/// for this SELECT run to completion (`duckdb.connect().sql(select).fetchdf()` raises
/// `InvalidInputException: Invalid Input Error: late failure`). Python's own export loop reads
/// with `execute` and `fetchmany`, and for a failure after the first chunk it prefixes the text
/// with "Invalid Input Error: Attempting to execute an unsuccessful or closed pending query
/// result\nError: "; the Rust export returns DuckDB's message without that prefix, as it did
/// before rows were streamed.
const LATE_FAILURE: &str = "Invalid Input Error: late failure";

#[test]
fn rows_arrive_in_several_batches_in_order() {
    let conn = Connection::open_in_memory().unwrap();
    let mut batches = 0;
    let mut seen = Vec::new();

    for_each_batch(
        &conn,
        &format!("SELECT range AS n, range * 2 AS m, 'v' AS t FROM range({ROWS})"),
        |batch| {
            batches += 1;
            let column = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            seen.extend(column.iter().flatten());
            Ok(())
        },
    )
    .unwrap();

    assert!(batches >= 2, "{batches} batch for {ROWS} rows");
    assert_eq!(seen, (0..ROWS).collect::<Vec<_>>());
}

#[test]
fn a_late_failure_is_reported_after_earlier_batches_were_delivered() {
    let conn = Connection::open_in_memory().unwrap();
    let mut batches = 0;

    let outcome: ApiResult<()> = for_each_batch(
        &conn,
        &format!(
            "SELECT CASE WHEN range = {} THEN error('late failure') ELSE range END AS n FROM range({LONG_RUN})",
            LONG_RUN - 100
        ),
        |_| {
            batches += 1;
            Ok(())
        },
    );

    assert!(batches >= 1, "the failure came before any batch");
    let detail = outcome.expect_err("the failure is reported");
    let detail = detail.detail();
    assert_eq!(detail, format!("Invalid SQL query: {LATE_FAILURE}"));
}

#[test]
fn a_stop_that_repeats_carries_duckdbs_message() {
    let error = stopped_early(|| {
        Err(duckdb::Error::DuckDBFailure(
            duckdb::ffi::Error::new(1),
            Some("boom".into()),
        ))
    });

    assert_eq!(error.detail(), "Invalid SQL query: boom");
}

#[test]
fn a_stop_that_does_not_repeat_keeps_the_generic_text() {
    let error = stopped_early(|| Ok(()));

    assert_eq!(
        error.detail(),
        "Invalid SQL query: the result stopped before its last row"
    );
}

#[test]
fn an_unrunnable_query_fails_before_any_batch() {
    let conn = Connection::open_in_memory().unwrap();
    let mut batches = 0;

    let outcome = for_each_batch(&conn, "SELECT * FROM missing_table", |_| {
        batches += 1;
        Ok(())
    });

    assert_eq!(batches, 0);
    assert!(outcome.is_err());
}

/// A batch of `values` with the flag column set on the rows listed in `closing`.
fn flagged(values: &[i64], closing: &[usize]) -> RecordBatch {
    let flags: Vec<bool> = (0..values.len())
        .map(|row| closing.contains(&row))
        .collect();
    RecordBatch::try_from_iter([
        ("n", Arc::new(Int64Array::from(values.to_vec())) as ArrayRef),
        ("flag", Arc::new(BooleanArray::from(flags)) as ArrayRef),
    ])
    .unwrap()
}

#[test]
fn the_closing_row_is_dropped_and_marks_the_stream_complete() {
    let mut is_complete = false;
    let rows = data_rows(&flagged(&[1, 2, 0], &[2]), &mut is_complete).unwrap();
    assert!(is_complete);
    assert_eq!(rows.num_columns(), 1);
    assert_eq!(rows.num_rows(), 2);
}

#[test]
fn rows_after_the_closing_row_fail_instead_of_being_dropped() {
    let mut is_complete = false;
    let error = data_rows(&flagged(&[1, 0, 2], &[1]), &mut is_complete).unwrap_err();
    assert_eq!(error.status().as_u16(), 500);

    let mut is_complete = true;
    let error = data_rows(&flagged(&[3], &[]), &mut is_complete).unwrap_err();
    assert_eq!(error.status().as_u16(), 500);
}
