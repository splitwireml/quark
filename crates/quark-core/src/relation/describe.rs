//! Lists a database's datasets and describes the columns of a relation.

use duckdb::{Connection, ToSql};
use serde::Serialize;

use crate::shared::columns::ColumnMeta;
use crate::shared::ids::dataset_id;

/// One table or view in an engine, as listed by `GET /datasets`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub schema: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// Lists user tables and views, ordered by schema then name.
pub fn datasets(conn: &Connection) -> duckdb::Result<Vec<DatasetInfo>> {
    // The attached columnar caches hold the same rows as the views over them.
    let mut statement = conn.prepare(
        "SELECT schema_name, table_name, 'TABLE' FROM duckdb_tables()
         WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog', 'quark_results')
           AND NOT regexp_full_match(database_name, 'cache_[0-9a-f]{16}')
         UNION ALL
         SELECT schema_name, view_name, 'VIEW' FROM duckdb_views()
         WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog', 'quark_results')
           AND NOT regexp_full_match(database_name, 'cache_[0-9a-f]{16}')
         ORDER BY 1, 2",
    )?;
    statement
        .query_map([], |row| {
            let schema: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok(DatasetInfo {
                id: dataset_id(&schema, &name),
                name,
                schema,
                kind: row.get(2)?,
            })
        })?
        .collect()
}

/// Column names and types of `relation`, which may contain `?` placeholders bound from `params`.
pub fn describe(
    conn: &Connection,
    relation: &str,
    params: &[&dyn ToSql],
) -> duckdb::Result<Vec<ColumnMeta>> {
    let mut statement = conn.prepare(&format!("DESCRIBE SELECT * FROM {relation}"))?;
    statement
        .query_map(params, |row| {
            Ok(ColumnMeta {
                name: row.get(0)?,
                type_name: row.get(1)?,
            })
        })?
        .collect()
}

#[cfg(test)]
mod tests;
