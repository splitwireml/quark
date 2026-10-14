//! Profile of a numeric column: counts, finite-value statistics and a histogram.

use std::collections::HashMap;

use duckdb::Connection;
use serde_json::{Value, json};

use super::{aggregate_row, bin_count, count_of, null_fraction};
use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::relation::rows::json_rows;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;
use crate::shared::types::is_integer;

pub fn profile(
    conn: &Connection,
    relation: &Relation,
    column: &ColumnMeta,
) -> duckdb::Result<Value> {
    let field = quote_ident(&column.name);
    let finite = format!("FILTER (WHERE isfinite({field}::DOUBLE))");
    let stats = aggregate_row(
        conn,
        relation,
        &format!(
            "count(*), count({field}), count(*) - count({field}), count(*) {finite}, \
             min({field}) {finite}, max({field}) {finite}, avg({field}) {finite}, \
             stddev_samp({field}) {finite}, quantile_cont({field}, 0.25) {finite}, \
             quantile_cont({field}, 0.5) {finite}, quantile_cont({field}, 0.75) {finite}"
        ),
    )?;
    let finite_count = count_of(&stats[3]);
    let histogram = if finite_count > 0 && !stats[4].is_null() && !stats[5].is_null() {
        let bins = bin_count(finite_count);
        histogram(conn, relation, column, &stats, bins)?
    } else {
        Vec::new()
    };
    Ok(json!({
        "kind": "numeric",
        "type": column.type_name,
        "row_count": stats[0],
        "non_null_count": stats[1],
        "null_count": stats[2],
        "null_fraction": null_fraction(count_of(&stats[2]), count_of(&stats[0])),
        "min": stats[4],
        "max": stats[5],
        "mean": stats[6],
        "stddev": stats[7],
        "p25": stats[8],
        "median": stats[9],
        "p75": stats[10],
        "histogram": histogram,
    }))
}

/// Picks the binning: `ntile` for integers past 2^53 - 1, one bin for a constant column, else equal width.
fn histogram(
    conn: &Connection,
    relation: &Relation,
    column: &ColumnMeta,
    stats: &[Value],
    bins: u64,
) -> duckdb::Result<Vec<Value>> {
    let field = quote_ident(&column.name);
    // Integers beyond 2^53 - 1 reach us as strings, as Python's `safe()` renders them.
    let is_unsafe = stats[4].is_string() || stats[5].is_string();
    if is_integer(&column.type_name) && is_unsafe {
        return ntile_bins(conn, relation, &field, bins);
    }
    let (Some(min), Some(max)) = (stats[4].as_f64(), stats[5].as_f64()) else {
        return Ok(Vec::new());
    };
    if min == max {
        return Ok(vec![
            json!({"lower": stats[4], "upper": stats[5], "count": stats[3]}),
        ]);
    }
    let width = (max - min) / bins as f64;
    let counts = bucket_counts(conn, relation, &field, bins, &stats[4], width)?;
    Ok((0..bins)
        .map(|index| {
            let upper = if index == bins - 1 {
                max
            } else {
                min + (index + 1) as f64 * width
            };
            json!({
                "lower": min + index as f64 * width,
                "upper": upper,
                "count": counts.get(&index).copied().unwrap_or(0),
            })
        })
        .collect())
}

fn ntile_bins(
    conn: &Connection,
    relation: &Relation,
    field: &str,
    bins: u64,
) -> duckdb::Result<Vec<Value>> {
    let sql = format!(
        "WITH ranked AS (SELECT {field}, ntile({bins}) OVER (ORDER BY {field}) AS bin \
         FROM {} WHERE {field} IS NOT NULL) \
         SELECT min({field}), max({field}), count(*) FROM ranked GROUP BY bin ORDER BY bin",
        relation.sql
    );
    let (_, rows) = json_rows(conn, &sql, &relation.params)?;
    Ok(rows
        .into_iter()
        .map(|row| json!({"lower": row[0], "upper": row[1], "count": row[2]}))
        .collect())
}

/// Row counts per equal-width bin index; the bound parameters precede the relation's.
fn bucket_counts(
    conn: &Connection,
    relation: &Relation,
    field: &str,
    bins: u64,
    min: &Value,
    width: f64,
) -> duckdb::Result<HashMap<u64, u64>> {
    let sql = format!(
        "SELECT least(?, floor(({field} - ?) / ?)::INTEGER), count(*) FROM {} \
         WHERE {field} IS NOT NULL AND isfinite({field}::DOUBLE) GROUP BY 1 ORDER BY 1",
        relation.sql
    );
    let params: Vec<Value> = [json!(bins - 1), min.clone(), json!(width)]
        .into_iter()
        .chain(relation.params.iter().cloned())
        .collect();
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(bind(&params), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.map(|row| row.map(|(index, count)| (index.unsigned_abs(), count.unsigned_abs())))
        .collect()
}

#[cfg(test)]
mod tests;
