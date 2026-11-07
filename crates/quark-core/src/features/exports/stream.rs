//! Reads a sheet's rows from DuckDB one Arrow batch at a time, so no writer holds the whole result.

use duckdb::Connection;
use duckdb::arrow::array::{Array, BooleanArray};
use duckdb::arrow::record_batch::RecordBatch;

use super::service::invalid_query;
use crate::error::{ApiError, ApiResult};
use crate::relation::params::bind;

/// The flag column of the closing row. A stream that fails midway ends like one that finished,
/// with no error to read, so only this row proves every row arrived.
const END_FLAG: &str = "__quark_export_end__";

/// Runs the SELECT `select_sql` in streaming mode and hands each batch to `each` in order; a
/// batch is dropped once `each` returns. The schema comes from a `LIMIT 0` probe, which plans the
/// query without running it. A stream that stops before its closing row is a 422 that carries
/// DuckDB's own message, found by running the SELECT once more (see `stopped_early`).
pub(super) fn for_each_batch(
    conn: &Connection,
    select_sql: &str,
    mut each: impl FnMut(RecordBatch) -> ApiResult<()>,
) -> ApiResult<()> {
    let sql = format!(
        "SELECT *, false AS {END_FLAG} FROM ({select_sql}) UNION ALL BY NAME SELECT true AS {END_FLAG}"
    );
    let schema = conn
        .prepare(&format!("SELECT * FROM ({sql}) LIMIT 0"))
        .map_err(invalid_query)?
        .query_arrow(bind(&[]))
        .map_err(invalid_query)?
        .get_schema();
    let mut statement = conn.prepare(&sql).map_err(invalid_query)?;
    let mut is_complete = false;
    for batch in statement
        .stream_arrow(bind(&[]), schema)
        .map_err(invalid_query)?
    {
        let rows = data_rows(&batch, &mut is_complete)?;
        if rows.num_rows() > 0 {
            each(rows)?;
        }
    }
    if is_complete {
        return Ok(());
    }
    Err(stopped_early(|| {
        conn.prepare(select_sql)?
            .query_arrow(bind(&[]))?
            .for_each(drop);
        Ok(())
    }))
}

/// The 422 for a stream that ended without its closing row. duckdb-rs `stream_arrow` hides the
/// error behind a normal end of stream, its FFI is private and `unsafe_code` is denied, so the
/// message is read by running the SELECT again with `rerun`, which materialises it as before the
/// streaming change. That costs time and memory only on this failure path. If the rerun fails,
/// its error is returned as every export error is; if it succeeds the failure did not repeat and
/// the text is generic.
fn stopped_early(rerun: impl FnOnce() -> duckdb::Result<()>) -> ApiError {
    match rerun() {
        Err(error) => invalid_query(error),
        Ok(()) => {
            ApiError::unprocessable("Invalid SQL query: the result stopped before its last row")
        }
    }
}

/// The batch without its flag column and closing row; `is_complete` turns true at the closing row.
/// That row comes last because DuckDB keeps `UNION ALL` children in order under its default
/// `preserve_insertion_order`. A row after it would mean that order broke, so the export fails
/// instead of dropping the rows that follow.
fn data_rows(batch: &RecordBatch, is_complete: &mut bool) -> ApiResult<RecordBatch> {
    let flag_index = batch.num_columns() - 1;
    let flags = batch
        .column(flag_index)
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| ApiError::internal("Internal error"))?;
    let closing = flags.iter().position(|flag| flag == Some(true));
    let is_after_closing = (*is_complete && batch.num_rows() > 0)
        || closing.is_some_and(|at| at + 1 != batch.num_rows());
    if is_after_closing {
        tracing::error!("export rows arrived after the closing row");
        return Err(ApiError::internal("Internal error"));
    }
    *is_complete |= closing.is_some();
    let kept: Vec<usize> = (0..flag_index).collect();
    let rows = batch.project(&kept).map_err(|error| {
        tracing::error!(%error, "could not drop the export flag column");
        ApiError::internal("Internal error")
    })?;
    Ok(rows.slice(0, closing.unwrap_or(batch.num_rows())))
}

#[cfg(test)]
mod tests;
