//! Writes one sheet as Parquet or JSON: Arrow batches go into a sandboxed in-memory writer that copies them out.

use std::path::Path;

use duckdb::Connection;

use super::model::{ExportFormat, JsonLayout};
use super::service::{PreparedSheet, invalid_query};
use super::stream::for_each_batch;
use crate::error::{ApiError, ApiResult};
use crate::runtime::dirs::lock_down_dirs;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::{quote_ident, sql_string};

/// Copies the sheet into a fresh in-memory writer that can reach only `dir`, then lets DuckDB
/// write `target`. The writer never runs user SQL: it only sees the sheet's own batches.
pub(super) fn write_copy(
    reader: &Connection,
    sheet: &PreparedSheet,
    format: ExportFormat,
    layout: JsonLayout,
    target: &Path,
    dir: &Path,
) -> ApiResult<()> {
    if has_repeated_name(&sheet.header) {
        let label = if format == ExportFormat::Parquet {
            "PARQUET"
        } else {
            "JSON"
        };
        return Err(ApiError::unprocessable(format!(
            "{label} export requires unique column names"
        )));
    }
    let writer = Connection::open_in_memory().map_err(copy_failed)?;
    lock_down_dirs(&writer, &[dir.to_string_lossy().into_owned()]).map_err(|error| {
        tracing::error!(%error, "could not lock down the export writer");
        ApiError::internal("Internal error")
    })?;
    // Types come from the wrapped relation; names come from the SQL's own header, as in Python.
    // A type the appender would change or refuse travels as text and is cast back on the way out.
    let definitions: Vec<String> = sheet
        .header
        .iter()
        .zip(&sheet.columns)
        .map(|(name, meta)| {
            let kind = if travels_as_text(&meta.type_name) {
                "VARCHAR"
            } else {
                &meta.type_name
            };
            format!("{} {kind}", quote_ident(name))
        })
        .collect();
    writer
        .execute_batch(&format!(
            "CREATE TABLE export_data({})",
            definitions.join(", ")
        ))
        .map_err(invalid_query)?;
    let mut appender = writer.appender("export_data").map_err(invalid_query)?;
    let select = format!(
        "SELECT {} FROM ({})",
        reader_select(&sheet.columns),
        sheet.sql
    );
    for_each_batch(reader, &select, |batch| {
        appender.append_record_batch(batch).map_err(invalid_query)
    })?;
    appender.flush().map_err(invalid_query)?;
    drop(appender);
    writer
        .execute_batch(&copy_statement(sheet, format, layout, target))
        .map_err(invalid_query)
}

fn copy_statement(
    sheet: &PreparedSheet,
    format: ExportFormat,
    layout: JsonLayout,
    target: &Path,
) -> String {
    let target = sql_string(&target.to_string_lossy());
    let columns = sheet.header.iter().zip(&sheet.columns);
    let options = match (format, layout) {
        (ExportFormat::Parquet, _) => "FORMAT PARQUET",
        (_, JsonLayout::Rows) => "FORMAT JSON, ARRAY true",
        (_, JsonLayout::Columns) => "FORMAT JSON",
    };
    let projection: Vec<String> = if layout == JsonLayout::Columns
        && format != ExportFormat::Parquet
    {
        columns
            .map(|(name, meta)| {
                format!(
                    "COALESCE(list({}), CAST([] AS {}[])) AS {}",
                    restore_expr(name, meta),
                    meta.type_name,
                    quote_ident(name)
                )
            })
            .collect()
    } else {
        columns
            .map(|(name, meta)| format!("{} AS {}", restore_expr(name, meta), quote_ident(name)))
            .collect()
    };
    format!(
        "COPY (SELECT {} FROM export_data) TO {target} ({options})",
        projection.join(", ")
    )
}

/// The reader's select list: `*`, with every column that travels as text cast to `VARCHAR`.
fn reader_select(columns: &[ColumnMeta]) -> String {
    let casts: Vec<String> = columns
        .iter()
        .filter(|meta| travels_as_text(&meta.type_name))
        .map(|meta| {
            let name = quote_ident(&meta.name);
            format!("CAST({name} AS VARCHAR) AS {name}")
        })
        .collect();
    if casts.is_empty() {
        "*".to_owned()
    } else {
        format!("* REPLACE ({})", casts.join(", "))
    }
}

/// The writer's value for a column: text columns are cast back to the type DESCRIBE reported.
fn restore_expr(name: &str, meta: &ColumnMeta) -> String {
    let name = quote_ident(name);
    if travels_as_text(&meta.type_name) {
        format!("CAST({name} AS {})", meta.type_name)
    } else {
        name
    }
}

/// Types the Arrow appender changes without an error (HUGEINT, BIT) or refuses (UHUGEINT, ENUM,
/// VARINT/BIGNUM, TIMETZ), at any nesting depth. Quoted names and literals are not type words.
fn travels_as_text(type_name: &str) -> bool {
    let mut word = String::new();
    let mut words = Vec::new();
    let mut quote = None;
    for ch in type_name.chars().chain(std::iter::once(' ')) {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_ascii_alphanumeric() || ch == '_' => word.push(ch.to_ascii_uppercase()),
            None if word.is_empty() => {}
            None => words.push(std::mem::take(&mut word)),
        }
    }
    let has = |text: &str| words.iter().any(|word| word == text);
    [
        "HUGEINT",
        "UHUGEINT",
        "BIT",
        "BITSTRING",
        "ENUM",
        "VARINT",
        "BIGNUM",
    ]
    .iter()
    .any(|text| has(text))
        || words
            .windows(4)
            .any(|w| w == ["TIME", "WITH", "TIME", "ZONE"])
}

fn has_repeated_name(names: &[String]) -> bool {
    let mut seen = std::collections::HashSet::with_capacity(names.len());
    !names.iter().all(|name| seen.insert(name.as_str()))
}

fn copy_failed(error: duckdb::Error) -> ApiError {
    tracing::error!(%error, "could not open the export writer");
    ApiError::internal("Internal error")
}

#[cfg(test)]
mod tests;
