//! Resolves a dataset id or a SQL text to the relation, columns and parameters a query runs over.

use duckdb::Connection;
use serde_json::Value;

use super::describe::{datasets, describe};
use super::guard::check_select;
use crate::error::{ApiError, ApiResult};
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Dataset(String),
    Sql(String),
}

/// What a target resolves to before the request's filters and sorts are applied.
pub(crate) struct Resolved {
    pub(crate) table: String,
    pub(crate) display_table: String,
    pub(crate) columns: Vec<ColumnMeta>,
    /// Physical flat-file cache table, only resolved for dedupe requests.
    pub(crate) rowid_table: Option<String>,
    /// Parameters for placeholders inside `table`, bound before the filters'.
    pub(crate) lead: Vec<Value>,
    /// The `sql` to report when the request adds no filters, sorts or dedupe.
    pub(crate) plain_sql: Option<String>,
    pub(crate) error_prefix: &'static str,
}

pub(crate) fn internal(_: duckdb::Error) -> ApiError {
    ApiError::internal("Internal error")
}

/// DuckDB parser failures mean the text is not a single SELECT; anything else is a bad query.
pub(crate) fn describe_error(error: duckdb::Error) -> ApiError {
    let text = error.to_string();
    if text.starts_with("Parser Error") {
        ApiError::unprocessable("SQL accepts only one read-only SELECT query")
    } else {
        ApiError::unprocessable(format!("Invalid SQL query: {text}"))
    }
}

pub(crate) fn resolve(conn: &Connection, target: &Target) -> ApiResult<Resolved> {
    resolve_query(conn, target, false)
}

pub(crate) fn resolve_query(
    conn: &Connection,
    target: &Target,
    dedupe: bool,
) -> ApiResult<Resolved> {
    match target {
        Target::Dataset(id) => {
            let item = datasets(conn)
                .map_err(internal)?
                .into_iter()
                .find(|item| item.id == *id)
                .ok_or_else(|| ApiError::not_found("Dataset not found"))?;
            let table = format!("{}.{}", quote_ident(&item.schema), quote_ident(&item.name));
            let columns = describe(conn, &table, &[]).map_err(internal)?;
            // A real column named rowid shadows DuckDB's physical row id.
            let rowid_table = if dedupe
                && item.kind == "VIEW"
                && !columns
                    .iter()
                    .any(|meta| meta.name.eq_ignore_ascii_case("rowid"))
            {
                cache_table(conn, &item.schema, &item.name)?
            } else {
                None
            };
            Ok(Resolved {
                display_table: table.clone(),
                table,
                columns,
                rowid_table,
                lead: Vec::new(),
                plain_sql: None,
                error_prefix: "Invalid filter value: ",
            })
        }
        Target::Sql(sql) => {
            let sql = check_select(conn, sql)?;
            let columns = describe(conn, "query(?)", &[&sql]).map_err(describe_error)?;
            let mut resolved = Resolved {
                table: "query(?)".to_owned(),
                display_table: format!("({sql})"),
                columns,
                rowid_table: None,
                lead: vec![Value::String(sql.clone())],
                plain_sql: Some(sql.clone()),
                error_prefix: "Invalid SQL query: ",
            };
            if dedupe
                && !resolved
                    .columns
                    .iter()
                    .any(|meta| meta.name.eq_ignore_ascii_case("rowid"))
            {
                for item in datasets(conn).map_err(internal)? {
                    let table =
                        format!("{}.{}", quote_ident(&item.schema), quote_ident(&item.name));
                    if item.kind == "VIEW" && sql == format!("SELECT * FROM {table}") {
                        if let Some(cache) = cache_table(conn, &item.schema, &item.name)? {
                            resolved.table = table;
                            resolved.rowid_table = Some(cache);
                            resolved.lead.clear();
                        }
                        break;
                    }
                }
            }
            Ok(resolved)
        }
    }
}

fn cache_table(conn: &Connection, schema: &str, name: &str) -> ApiResult<Option<String>> {
    let comment: Option<String> = conn
        .query_row(
            "SELECT max(v.comment) FROM duckdb_views() v JOIN duckdb_databases() d \
             ON v.comment = 'quark:columnar:\"' || d.database_name || '\".main.data' \
             WHERE v.database_name = current_database() AND v.schema_name = ? AND v.view_name = ?",
            [schema, name],
            |row| row.get(0),
        )
        .map_err(internal)?;
    Ok(comment.and_then(|comment| {
        let hash = comment
            .strip_prefix("quark:columnar:\"cache_")?
            .strip_suffix("\".main.data")?;
        (hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| format!("\"cache_{hash}\".main.data"))
    }))
}

#[cfg(test)]
mod tests;
