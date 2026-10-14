//! SQL for one page: the row count with null fractions, and the page's rows as Arrow batches.

use duckdb::Connection;
use duckdb::arrow::datatypes::SchemaRef;
use duckdb::arrow::record_batch::RecordBatch;
use serde_json::Value;

use crate::relation::params::bind;
use crate::relation::request::QueryRequest;
use crate::relation::rows::select_list;
use crate::runtime::cache::stats::StatsEntry;
use crate::shared::columns::{ColumnMeta, ColumnSummary};
use crate::shared::sql::quote_ident;
use crate::shared::types::{is_numeric, profile_kind};

/// The page's Arrow schema and batches.
pub(super) struct Fetched {
    pub(super) schema: SchemaRef,
    pub(super) batches: Vec<RecordBatch>,
}

/// Counts the relation's rows and measures each column's null fraction.
pub(super) fn compute_stats(
    conn: &Connection,
    relation: &str,
    columns: &[ColumnMeta],
    params: &[Value],
) -> duckdb::Result<StatsEntry> {
    let total_rows = conn.query_row(
        &format!("SELECT count(*) FROM {relation}"),
        bind(params),
        |row| row.get(0),
    )?;

    let null_fractions: Vec<f64> = if columns.is_empty() {
        Vec::new()
    } else {
        let averages: Vec<String> = columns
            .iter()
            .map(|meta| {
                format!(
                    "avg(CASE WHEN {} IS NULL THEN 1.0 ELSE 0.0 END)",
                    quote_ident(&meta.name)
                )
            })
            .collect();
        conn.query_row(
            &format!("SELECT {} FROM {relation}", averages.join(", ")),
            bind(params),
            |row| {
                (0..columns.len())
                    .map(|index| Ok(row.get::<_, Option<f64>>(index)?.unwrap_or(0.0)))
                    .collect()
            },
        )?
    };

    let columns = columns
        .iter()
        .zip(null_fractions)
        .map(|(meta, null_fraction)| ColumnSummary {
            name: meta.name.clone(),
            type_name: meta.type_name.clone(),
            numeric: is_numeric(&meta.type_name),
            profile_kind: profile_kind(&meta.type_name),
            null_fraction,
        })
        .collect();
    Ok(StatsEntry {
        columns,
        total_rows,
    })
}

pub(super) fn fetch_page(
    conn: &Connection,
    ordered: &str,
    columns: &[ColumnMeta],
    params: &[Value],
    request: &QueryRequest,
) -> duckdb::Result<Fetched> {
    let paging = [
        Value::from(request.page_size),
        Value::from((request.page - 1).saturating_mul(request.page_size)),
    ];
    let params: Vec<Value> = params.iter().cloned().chain(paging).collect();
    let mut statement = conn.prepare(&format!(
        "SELECT {} FROM ({ordered}) AS result LIMIT ? OFFSET ?",
        select_list(columns)
    ))?;
    let arrow = statement.query_arrow(bind(&params))?;
    let schema = arrow.get_schema();
    let batches = arrow.collect();
    Ok(Fetched { schema, batches })
}
