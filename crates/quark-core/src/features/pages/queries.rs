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

/// Counts can use storage metadata when a cache column has no nulls.
pub(super) fn source_stats(
    conn: &Connection,
    table: &str,
    columns: &[ColumnMeta],
) -> duckdb::Result<StatsEntry> {
    let counts = std::iter::once("count(*)".to_owned())
        .chain(
            columns
                .iter()
                .map(|meta| format!("count({})", quote_ident(&meta.name))),
        )
        .collect::<Vec<_>>()
        .join(", ");
    conn.query_row(&format!("SELECT {counts} FROM {table}"), [], |row| {
        let total_rows: u64 = row.get(0)?;
        let fractions = (0..columns.len())
            .map(|index| {
                let present: u64 = row.get(index + 1)?;
                Ok(if total_rows == 0 {
                    0.0
                } else {
                    (total_rows - present) as f64 / total_rows as f64
                })
            })
            .collect::<duckdb::Result<Vec<_>>>()?;
        Ok(StatsEntry {
            columns: summaries(columns, fractions),
            total_rows,
        })
    })
}

/// Counts the relation's rows and measures each column's null fraction.
pub(super) fn compute_stats(
    conn: &Connection,
    relation: &str,
    columns: &[ColumnMeta],
    params: &[Value],
    source: Option<&StatsEntry>,
) -> duckdb::Result<StatsEntry> {
    let total_rows = conn.query_row(
        &format!("SELECT count(*) FROM {relation}"),
        bind(params),
        |row| row.get(0),
    )?;

    // Selecting rows cannot introduce nulls into a source column that has none.
    let nullable: Vec<_> = columns
        .iter()
        .enumerate()
        .filter(|(index, meta)| {
            source.is_none_or(|stats| {
                stats
                    .columns
                    .get(*index)
                    .is_none_or(|column| column.name != meta.name || column.null_fraction != 0.0)
            })
        })
        .collect();
    let mut null_fractions = vec![0.0; columns.len()];
    if !nullable.is_empty() {
        let averages: Vec<String> = nullable
            .iter()
            .map(|(_, meta)| {
                format!(
                    "avg(CASE WHEN {} IS NULL THEN 1.0 ELSE 0.0 END)",
                    quote_ident(&meta.name)
                )
            })
            .collect();
        let measured: Vec<f64> = conn.query_row(
            &format!("SELECT {} FROM {relation}", averages.join(", ")),
            bind(params),
            |row| {
                (0..nullable.len())
                    .map(|index| Ok(row.get::<_, Option<f64>>(index)?.unwrap_or(0.0)))
                    .collect()
            },
        )?;
        for ((index, _), fraction) in nullable.into_iter().zip(measured) {
            null_fractions[index] = fraction;
        }
    }

    Ok(StatsEntry {
        columns: summaries(columns, null_fractions),
        total_rows,
    })
}

fn summaries(columns: &[ColumnMeta], null_fractions: Vec<f64>) -> Vec<ColumnSummary> {
    columns
        .iter()
        .zip(null_fractions)
        .map(|(meta, null_fraction)| ColumnSummary {
            name: meta.name.clone(),
            type_name: meta.type_name.clone(),
            numeric: is_numeric(&meta.type_name),
            profile_kind: profile_kind(&meta.type_name),
            null_fraction,
        })
        .collect()
}

pub(super) fn fetch_page(
    conn: &Connection,
    ordered: &str,
    page: Option<&str>,
    columns: &[ColumnMeta],
    params: &[Value],
    request: &QueryRequest,
) -> duckdb::Result<Fetched> {
    let paging = [
        Value::from(request.page_size),
        Value::from((request.page - 1).saturating_mul(request.page_size)),
    ];
    let params: Vec<Value> = params.iter().cloned().chain(paging).collect();
    let select = select_list(columns);
    let query = match page {
        Some(page) => format!("SELECT {select} FROM ({page}) AS result"),
        None => format!("SELECT {select} FROM ({ordered}) AS result LIMIT ? OFFSET ?"),
    };
    let mut statement = conn.prepare(&query)?;
    let arrow = statement.query_arrow(bind(&params))?;
    let schema = arrow.get_schema();
    let batches = arrow.collect();
    Ok(Fetched { schema, batches })
}
