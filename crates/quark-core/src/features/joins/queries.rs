//! SQL for a join preview: row counts, key uniqueness and the inner-join count.

use duckdb::Connection;

use crate::shared::sql::quote_ident;

pub(super) fn row_count(conn: &Connection, table: &str) -> duckdb::Result<u64> {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
        row.get(0)
    })
}

/// Whether the non-null key tuples of `table` are all different.
pub(super) fn has_unique_keys(
    conn: &Connection,
    table: &str,
    keys: &[String],
) -> duckdb::Result<bool> {
    let fields: Vec<String> = keys.iter().map(|key| quote_ident(key)).collect();
    let distinct = match fields.as_slice() {
        [only] => only.clone(),
        _ => format!("({})", fields.join(", ")),
    };
    let present: Vec<String> = fields
        .iter()
        .map(|field| format!("{field} IS NOT NULL"))
        .collect();
    let (non_null, distinct_count): (u64, u64) = conn.query_row(
        &format!(
            "SELECT count(*), count(DISTINCT {distinct}) FROM {table} WHERE {}",
            present.join(" AND ")
        ),
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(non_null == distinct_count)
}

pub(super) fn inner_join_rows(
    conn: &Connection,
    left: &str,
    right: &str,
    left_keys: &[String],
    right_keys: &[String],
) -> duckdb::Result<u64> {
    let on: Vec<String> = left_keys
        .iter()
        .zip(right_keys)
        .map(|(l, r)| format!("l.{} = r.{}", quote_ident(l), quote_ident(r)))
        .collect();
    conn.query_row(
        &format!(
            "SELECT count(*) FROM {left} l INNER JOIN {right} r ON {}",
            on.join(" AND ")
        ),
        [],
        |row| row.get(0),
    )
}
