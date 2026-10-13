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
    match target {
        Target::Dataset(id) => {
            let item = datasets(conn)
                .map_err(internal)?
                .into_iter()
                .find(|item| item.id == *id)
                .ok_or_else(|| ApiError::not_found("Dataset not found"))?;
            let table = format!("{}.{}", quote_ident(&item.schema), quote_ident(&item.name));
            let columns = describe(conn, &table, &[]).map_err(internal)?;
            Ok(Resolved {
                display_table: table.clone(),
                table,
                columns,
                lead: Vec::new(),
                plain_sql: None,
                error_prefix: "Invalid filter value: ",
            })
        }
        Target::Sql(sql) => {
            let sql = check_select(conn, sql)?;
            let columns = describe(conn, "query(?)", &[&sql]).map_err(describe_error)?;
            Ok(Resolved {
                table: "query(?)".to_owned(),
                display_table: format!("({sql})"),
                columns,
                lead: vec![Value::String(sql.clone())],
                plain_sql: Some(sql),
                error_prefix: "Invalid SQL query: ",
            })
        }
    }
}

#[cfg(test)]
mod tests;
