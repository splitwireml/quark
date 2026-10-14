//! Profile of a categorical column: counts, distinct values and the twenty most frequent.

use duckdb::Connection;
use serde_json::{Value, json};

use super::{aggregate_row, count_of, null_fraction};
use crate::relation::filtered::Relation;
use crate::relation::rows::json_rows;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;

const TOP_VALUE_LIMIT: usize = 20;

pub fn profile(
    conn: &Connection,
    relation: &Relation,
    column: &ColumnMeta,
) -> duckdb::Result<Value> {
    let field = quote_ident(&column.name);
    let counts = aggregate_row(
        conn,
        relation,
        &format!("count(*), count({field}), count(*) - count({field}), count(DISTINCT {field})"),
    )?;
    let sql = format!(
        "SELECT {field}, count(*) AS count FROM {} WHERE {field} IS NOT NULL \
         GROUP BY {field} ORDER BY count DESC, {field} LIMIT {TOP_VALUE_LIMIT}",
        relation.sql
    );
    let (_, top) = json_rows(conn, &sql, &relation.params)?;
    let top_values: Vec<Value> = top
        .into_iter()
        .map(|row| json!({"value": row[0], "count": row[1]}))
        .collect();
    Ok(json!({
        "kind": "categorical",
        "type": column.type_name,
        "row_count": counts[0],
        "non_null_count": counts[1],
        "null_count": counts[2],
        "null_fraction": null_fraction(count_of(&counts[2]), count_of(&counts[0])),
        "distinct_count": counts[3],
        "top_values": top_values,
    }))
}
