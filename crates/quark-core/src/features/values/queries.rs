//! SQL for category values: one page of distinct values with counts, and the distinct total.

use duckdb::Connection;
use serde_json::Value;

use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::shared::sql::quote_ident;

/// One page of `(value, count)` rows plus the distinct total, which is `None` for an empty page.
pub(super) struct Fetched {
    pub(super) rows: Vec<(String, i64)>,
    pub(super) total: Option<i64>,
}

fn condition(field: &str) -> String {
    format!("{field} IS NOT NULL AND contains(lower({field}), lower(?))")
}

/// Bound in the order the placeholders appear: the relation's, the search, then any extras.
fn parameters(relation: &Relation, search: &str, extra: &[i64]) -> Vec<Value> {
    let mut params = relation.params.clone();
    params.push(Value::from(search));
    params.extend(extra.iter().map(|number| Value::from(*number)));
    params
}

/// The page ordered by count then value; the window over the grouped rows is the distinct total.
pub(super) fn fetch_values(
    conn: &Connection,
    relation: &Relation,
    column: &str,
    search: &str,
    limit: i64,
    offset: i64,
) -> duckdb::Result<Fetched> {
    let field = quote_ident(column);
    let mut statement = conn.prepare(&format!(
        "SELECT {field}, count(*) AS count, count(*) OVER () AS total \
         FROM {} WHERE {} GROUP BY {field} ORDER BY count DESC, {field} LIMIT ? OFFSET ?",
        relation.sql,
        condition(&field),
    ))?;
    let params = parameters(relation, search, &[limit, offset]);
    let rows = statement
        .query_map(bind(&params), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<duckdb::Result<Vec<_>>>()?;
    let total = rows.first().map(|(_, _, total)| *total);
    Ok(Fetched {
        rows: rows
            .into_iter()
            .map(|(value, count, _)| (value, count))
            .collect(),
        total,
    })
}

/// The number of distinct values matching `search`.
pub(super) fn count_distinct(
    conn: &Connection,
    relation: &Relation,
    column: &str,
    search: &str,
) -> duckdb::Result<i64> {
    let field = quote_ident(column);
    let params = parameters(relation, search, &[]);
    conn.query_row(
        &format!(
            "SELECT count(DISTINCT {field}) FROM {} WHERE {}",
            relation.sql,
            condition(&field)
        ),
        bind(&params),
        |row| row.get(0),
    )
}
