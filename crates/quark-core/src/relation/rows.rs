//! Runs a relation and returns its columns and every row as JSON values under Python's `safe()` rules.

use duckdb::Connection;
use serde_json::Value;

use super::params::bind;
use crate::shared::columns::ColumnMeta;
use crate::shared::encode::cell::cell_json;

/// Types come from `DESCRIBE`, not Arrow, so DuckDB's own type names reach `cell_json`.
pub fn json_rows(
    conn: &Connection,
    sql: &str,
    params: &[Value],
) -> duckdb::Result<(Vec<ColumnMeta>, Vec<Vec<Value>>)> {
    let mut describe = conn.prepare(&format!("DESCRIBE SELECT * FROM ({sql})"))?;
    let columns = describe
        .query_map(bind(params), |row| {
            Ok(ColumnMeta {
                name: row.get(0)?,
                type_name: row.get(1)?,
            })
        })?
        .collect::<duckdb::Result<Vec<_>>>()?;

    let mut statement = conn.prepare(&format!("SELECT * FROM ({sql})"))?;
    let mut rows = Vec::new();
    for batch in statement.query_arrow(bind(params))? {
        for row in 0..batch.num_rows() {
            rows.push(
                columns
                    .iter()
                    .zip(batch.columns())
                    .map(|(meta, array)| {
                        cell_json(array.as_ref(), row, &meta.type_name, &chrono::Local)
                    })
                    .collect(),
            );
        }
    }
    Ok((columns, rows))
}

#[cfg(test)]
mod tests;
