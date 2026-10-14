//! Executes one page of a dataset or SQL query: counts, null fractions and JSON rows.

use std::sync::atomic::Ordering;
use std::time::Instant;

use duckdb::Connection;
use duckdb::arrow::record_batch::RecordBatch;
use serde_json::{Map, Value};

use super::model::{Format, PageBody, PageMeta};
use super::queries::{compute_stats, fetch_page};
use crate::error::{ApiError, ApiResult};
use crate::relation::builder::build_query;
use crate::relation::request::QueryRequest;
use crate::relation::target::{Target, resolve};
use crate::runtime::cache::results::{RESULTS_SCHEMA, ResultAction, drop_statement, is_plain_scan};
use crate::runtime::cache::stats::stats_key;
use crate::runtime::engine::Engine;
use crate::runtime::mount::ViewInfo;
use crate::shared::columns::ColumnMeta;
use crate::shared::encode::arrow::encode_page;
use crate::shared::encode::cell::cell_json;
use crate::shared::naming::page_count;

fn rows_json(batches: &[RecordBatch], columns: &[ColumnMeta]) -> Vec<Value> {
    let mut rows = Vec::with_capacity(batches.iter().map(RecordBatch::num_rows).sum());
    for batch in batches {
        for row in 0..batch.num_rows() {
            let object: Map<String, Value> = columns
                .iter()
                .zip(batch.columns())
                .map(|(meta, array)| {
                    let cell = cell_json(array.as_ref(), row, &meta.type_name, &chrono::Local);
                    (meta.name.clone(), cell)
                })
                .collect();
            rows.push(Value::Object(object));
        }
    }
    rows
}

/// Runs one page of `target` with the request's filters, sorts and dedupe applied, on `conn`.
/// The engine's caches are locked one entry at a time, never while a query runs.
pub fn run_page(
    engine: &Engine,
    conn: &Connection,
    target: &Target,
    request: &QueryRequest,
    format: Format,
) -> ApiResult<PageBody> {
    let started = Instant::now();
    request.validate()?;
    let resolved = resolve(conn, target)?;
    let built = build_query(
        &resolved.table,
        &resolved.display_table,
        &resolved.columns,
        request,
    )?;
    let unprocessable =
        |error: duckdb::Error| ApiError::unprocessable(format!("{}{error}", resolved.error_prefix));

    let mut params = resolved.lead.clone();
    params.extend(built.params.iter().cloned());
    let has_controls = !request.filters.is_empty()
        || !request.sorts.is_empty()
        || !request.dedupe_columns.is_empty();
    // A scan with no controls reads its table or view as it is; a result table would only copy it.
    let is_plain = match &resolved.plain_sql {
        Some(sql) => {
            let mounted: Vec<ViewInfo> = engine
                .primary()
                .mounted
                .values()
                .flatten()
                .cloned()
                .collect();
            is_plain_scan(request, sql, &mounted)
        }
        None => !has_controls,
    };
    let result_key = stats_key(&built.ordered, &params);
    let action = if is_plain {
        ResultAction::ServeDirect
    } else {
        engine.results().note_request(&result_key)
    };
    let evicted = engine.results().take_dropped();
    if !evicted.is_empty() {
        let primary = engine.primary();
        for table in evicted {
            if let Err(error) = primary.conn.execute_batch(&drop_statement(&table)) {
                tracing::warn!(table, %error, "could not drop an evicted result table");
            }
        }
    }
    let saved = match &action {
        ResultAction::ServeFrom(table) => Some(format!("{RESULTS_SCHEMA}.{table}")),
        _ => None,
    };
    let saved_scan = saved.as_ref().map(|table| format!("SELECT * FROM {table}"));
    let (relation, ordered, bound) = match (&saved, &saved_scan) {
        (Some(table), Some(scan)) => (table.as_str(), scan.as_str(), [].as_slice()),
        _ => (
            built.relation.as_str(),
            built.ordered.as_str(),
            params.as_slice(),
        ),
    };

    let key = stats_key(&built.relation, &params);
    let cached = engine.stats().get(&key);
    let stats = match cached {
        Some(stats) => stats,
        None => {
            engine
                .counters
                .count_queries
                .fetch_add(1, Ordering::Relaxed);
            let stats =
                compute_stats(conn, relation, &resolved.columns, bound).map_err(unprocessable)?;
            engine.stats().insert(key, stats.clone());
            stats
        }
    };
    let fetched =
        fetch_page(conn, ordered, &resolved.columns, bound, request).map_err(unprocessable)?;
    if let ResultAction::StartBuild(table) = &action {
        let primary = engine.primary();
        engine
            .results()
            .start_build(&primary.conn, &result_key, table, &built.ordered, params);
    }

    let sql = match resolved.plain_sql {
        Some(plain) if !has_controls => plain,
        _ => built.display,
    };
    let meta = PageMeta {
        columns: stats.columns,
        page: request.page,
        page_size: request.page_size,
        total_rows: stats.total_rows,
        // validate() guarantees page_size >= 1.
        total_pages: page_count(stats.total_rows, request.page_size.unsigned_abs()),
        elapsed_ms: (started.elapsed().as_secs_f64() * 1_000_000.0).round() / 1000.0,
        sql,
    };
    let Ok(Value::Object(mut body)) = serde_json::to_value(&meta) else {
        return Err(ApiError::internal("Internal error"));
    };
    match format {
        Format::Json => {
            let rows = rows_json(&fetched.batches, &resolved.columns);
            body.insert("rows".to_owned(), Value::Array(rows));
            Ok(PageBody::Json(Value::Object(body)))
        }
        Format::Arrow => encode_page(&fetched.schema, &fetched.batches, &resolved.columns, body)
            .map(PageBody::Arrow),
    }
}

#[cfg(test)]
mod tests;
