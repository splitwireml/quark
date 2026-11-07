//! Profile of a date column: counts, range, a square-root-rule histogram and counts per year.

use duckdb::Connection;
use serde_json::{Value, json};

use super::{aggregate_row, bin_count, count_of, null_fraction};
use crate::relation::filtered::Relation;
use crate::relation::rows::json_rows;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;

pub fn profile(
    conn: &Connection,
    relation: &Relation,
    column: &ColumnMeta,
) -> duckdb::Result<Value> {
    let field = quote_ident(&column.name);
    let stats = aggregate_row(
        conn,
        relation,
        &format!(
            "count(*), count({field}), count(*) - count({field}), count(DISTINCT {field}), \
             min({field}), max({field})"
        ),
    )?;
    let non_null = count_of(&stats[1]);
    let histogram = if non_null > 0 {
        histogram(conn, relation, &field, bin_count(non_null))?
    } else {
        Vec::new()
    };
    // Python skips every type whose name starts with TIME, TIMESTAMP included.
    let year_counts = if column.type_name.to_uppercase().starts_with("TIME") {
        Vec::new()
    } else {
        year_counts(conn, relation, &field)?
    };
    Ok(json!({
        "kind": "date",
        "type": column.type_name,
        "row_count": stats[0],
        "non_null_count": stats[1],
        "null_count": stats[2],
        "null_fraction": null_fraction(count_of(&stats[2]), count_of(&stats[0])),
        "distinct_count": stats[3],
        "min": stats[4],
        "max": stats[5],
        "histogram": histogram,
        "year_counts": year_counts,
    }))
}

fn histogram(
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

fn year_counts(conn: &Connection, relation: &Relation, field: &str) -> duckdb::Result<Vec<Value>> {
    let sql = format!(
        "SELECT strftime({field}, '%Y'), count(*) FROM {} WHERE {field} IS NOT NULL \
         GROUP BY 1 ORDER BY 1",
        relation.sql
    );
    let (_, rows) = json_rows(conn, &sql, &relation.params)?;
    Ok(rows
        .into_iter()
        .map(|row| json!({"year": row[0], "count": row[1]}))
        .collect())
}
