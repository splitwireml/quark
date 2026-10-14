//! Column profiles for categorical and date columns, plus the helpers they share.

use duckdb::Connection;
use serde_json::Value;

use super::filtered::Relation;
use super::rows::json_rows;

pub mod categorical;
pub mod date;

/// Histogram bin count: the square root of the non-null count rounded up, between 1 and 20.
pub fn bin_count(non_null: u64) -> u64 {
    let root = non_null.isqrt();
    let ceiling = if root * root < non_null {
        root + 1
    } else {
        root
    };
    ceiling.clamp(1, 20)
}

/// Share of rows that are null; 0.0 for an empty relation.
pub fn null_fraction(nulls: u64, rows: u64) -> f64 {
    if rows == 0 {
        0.0
    } else {
        nulls as f64 / rows as f64
    }
}

/// The first row of `SELECT <select> FROM <relation>`, as JSON values.
fn aggregate_row(
    conn: &Connection,
    relation: &Relation,
    select: &str,
) -> duckdb::Result<Vec<Value>> {
    let sql = format!("SELECT {select} FROM {}", relation.sql);
    let (_, rows) = json_rows(conn, &sql, &relation.params)?;
    Ok(rows.into_iter().next().unwrap_or_default())
}

/// A non-negative count taken from a JSON number.
fn count_of(value: &Value) -> u64 {
    value.as_u64().unwrap_or_default()
}

#[cfg(test)]
mod tests;
