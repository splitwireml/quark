//! Finds the next or previous cell containing a term in a numbered copy of the View.

use std::collections::HashSet;

use duckdb::Connection;
use serde_json::Value;

use super::model::{CellSearch, FindResponse, FoundCell, SearchDirection};
use crate::error::{ApiError, ApiResult};
use crate::relation::params::bind;
use crate::relation::target::{Target, resolve};
use crate::runtime::cache::numbered::{NumberedState, NumberedUse, memory_statement};
use crate::runtime::cache::results::drop_statement;
use crate::runtime::engine::Engine;
use crate::shared::sql::quote_ident;

fn cannot_search(error: duckdb::Error) -> ApiError {
    ApiError::unprocessable(format!("Cannot search this View: {error}"))
}

/// Attaches the in-memory database and drops the tables a newer one pushed out, on the primary connection.
pub(super) fn prepare_memory(engine: &Engine, evicted: &[String]) -> ApiResult<()> {
    let primary = engine.primary();
    primary
        .conn
        .execute_batch(&memory_statement())
        .map_err(cannot_search)?;
    for table in evicted {
        let name = table.rsplit('.').next().unwrap_or(table);
        if let Err(error) = primary.conn.execute_batch(&drop_statement(name)) {
            tracing::warn!(table, %error, "could not drop an evicted numbered table");
        }
    }
    Ok(())
}

/// Where a search reads the numbered rows from.
pub(super) struct Source {
    /// Table name, or the name of the materialized live relation.
    from: String,
    /// `WITH` clause that numbers the View in place; empty when a table is read.
    prefix: String,
    /// Binds the View's SQL for the prefix, ahead of the search terms.
    params: Vec<Value>,
    /// A table whose entry was evicted while this search built it; dropped after the search.
    pub(super) orphan: Option<String>,
}

fn row_number_select(used: &NumberedUse) -> String {
    format!(
        "SELECT row_number() OVER () - 1 AS {}, * FROM query(?)",
        quote_ident(&used.row_key)
    )
}

/// Picks the numbered rows to read. The creator builds the table; a search that finds it building reads
/// the live numbered relation instead of creating or waiting. Only the creator marks the entry ready or forgets it.
pub(super) fn source_for(
    engine: &Engine,
    conn: &Connection,
    used: &NumberedUse,
    sql: &str,
) -> ApiResult<Source> {
    prepare_memory(engine, &used.evicted)?;
    let table = |orphan| Source {
        from: used.table.clone(),
        prefix: String::new(),
        params: Vec::new(),
        orphan,
    };
    match used.state {
        NumberedState::Ready => Ok(table(None)),
        NumberedState::Building => Ok(Source {
            from: "numbered".to_owned(),
            prefix: format!(
                "WITH numbered AS MATERIALIZED ({}) ",
                row_number_select(used)
            ),
            params: vec![Value::String(sql.to_owned())],
            orphan: None,
        }),
        NumberedState::Create => {
            let created = conn.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} AS {}",
                    used.table,
                    row_number_select(used)
                ),
                bind(&[Value::String(sql.to_owned())]),
            );
            if let Err(error) = created {
                engine.numbered().forget(&used.table);
                return Err(cannot_search(error));
            }
            let is_kept = engine.numbered().mark_ready(&used.table);
            Ok(table((!is_kept).then(|| used.table.clone())))
        }
    }
}

/// One `SELECT` per searched column: row, the column's place in the request, and its text.
fn matches_sql(used: &NumberedUse, source: &Source, columns: &[String]) -> String {
    let row = quote_ident(&used.row_key);
    let parts: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let text = format!("CAST({} AS VARCHAR)", quote_ident(name));
            format!(
                "SELECT {row} AS r, {index} AS c, {text} AS value FROM {} \
                 WHERE contains(lower({text}), lower(?))",
                source.from
            )
        })
        .collect();
    parts.join(" UNION ALL ")
}

/// Runs the search over `source`: the first match after the cursor in the direction, wrapping around.
fn search_source(
    conn: &Connection,
    request: &CellSearch,
    used: &NumberedUse,
    source: &Source,
) -> ApiResult<Option<FoundCell>> {
    let (comparison, order) = match request.direction {
        SearchDirection::Next => (">", "ASC"),
        SearchDirection::Previous => ("<", "DESC"),
    };
    let query = format!(
        "{}SELECT r, c, value FROM ({}) ORDER BY CASE WHEN (r, c) {comparison} (?, ?) THEN 0 ELSE 1 END, r {order}, c {order} LIMIT 1",
        source.prefix,
        matches_sql(used, source, &request.columns)
    );
    let mut params = source.params.clone();
    params.extend(vec![
        Value::String(request.term.clone());
        request.columns.len()
    ]);
    params.push(request.after_row.into());
    params.push(request.after_column.into());
    let mut statement = conn.prepare(&query).map_err(cannot_search)?;
    let mut rows = statement
        .query_map(bind(&params), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(cannot_search)?;
    let found = rows
        .next()
        .transpose()
        .map_err(cannot_search)?
        .and_then(|(row, index, value)| {
            let column_index = usize::try_from(index).ok()?;
            let column = request.columns.get(column_index)?.clone();
            Some(FoundCell {
                row,
                column,
                column_index,
                value,
            })
        });
    Ok(found)
}

/// Finds the first match after the cursor in the search direction, wrapping to the other end of the View.
pub fn find_cell(
    engine: &Engine,
    conn: &Connection,
    request: &CellSearch,
) -> ApiResult<FindResponse> {
    request.validate()?;
    let resolved = resolve(conn, &Target::Sql(request.sql.clone()))?;
    let known: HashSet<&str> = resolved
        .columns
        .iter()
        .map(|meta| meta.name.as_str())
        .collect();
    let unique: HashSet<&str> = request.columns.iter().map(String::as_str).collect();
    if unique.len() != request.columns.len() || !unique.is_subset(&known) {
        return Err(ApiError::unprocessable("Invalid search column"));
    }
    let sql = resolved.plain_sql.as_deref().unwrap_or(&request.sql);
    let used = engine
        .numbered()
        .touch(sql, resolved.columns.iter().map(|meta| meta.name.as_str()));
    let source = source_for(engine, conn, &used, sql)?;
    let found = search_source(conn, request, &used, &source);
    if let Some(orphan) = &source.orphan {
        prepare_memory(engine, std::slice::from_ref(orphan))?;
    }
    Ok(FindResponse { found: found? })
}
