//! Dataset metadata: table and sheet listings, and the physical cache behind a flat-file view.

use duckdb::{Connection, params};

use crate::relation::describe::DatasetInfo;
use crate::shared::ids::dataset_id;
use crate::shared::sql::sql_string;

/// Records a flat-file view's physical cache table without exposing its row id as a column.
pub(crate) fn mark_cache_view(conn: &Connection, target: &str, origin: &str) -> duckdb::Result<()> {
    if origin.starts_with("\"cache_") && origin.ends_with("\".main.data") {
        conn.execute_batch(&format!(
            "COMMENT ON VIEW {target} IS {}",
            sql_string(&format!("quark:columnar:{origin}"))
        ))?;
    }
    Ok(())
}

/// Tables and views of the database attached as `alias`, ordered by schema then name.
pub(super) fn attached_datasets(
    conn: &Connection,
    alias: &str,
) -> duckdb::Result<Vec<DatasetInfo>> {
    let mut statement = conn.prepare(
        "SELECT schema_name, table_name, 'TABLE' FROM duckdb_tables()
         WHERE database_name = ? AND NOT internal
           AND schema_name NOT IN ('information_schema', 'pg_catalog')
         UNION ALL
         SELECT schema_name, view_name, 'VIEW' FROM duckdb_views()
         WHERE database_name = ? AND NOT internal
           AND schema_name NOT IN ('information_schema', 'pg_catalog')
         ORDER BY 1, 2",
    )?;
    statement
        .query_map(params![alias, alias], |row| {
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

/// The sheets of a workbook as `TABLE` datasets in `main`, ordered by name like Python's listing.
pub(super) fn sheet_datasets(sheets: &[String]) -> Vec<DatasetInfo> {
    let mut items: Vec<_> = sheets
        .iter()
        .map(|sheet| DatasetInfo {
            id: dataset_id("main", sheet),
            name: sheet.clone(),
            schema: "main".to_owned(),
            kind: "TABLE".to_owned(),
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}
