//! Builds the filtered, deduplicated relation that analysis routes read, with its bound parameters.

use duckdb::Connection;
use serde_json::Value;

use super::builder::build_with_rowid;
use super::request::QueryRequest;
use super::target::{Target, resolve_query};
use crate::error::ApiResult;
use crate::shared::columns::ColumnMeta;

/// A target's filtered relation: `sql` is `(SELECT * FROM <table> WHERE … QUALIFY …)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub sql: String,
    /// The target's lead parameters (the SQL text for an SQL target), then the filter values.
    pub params: Vec<Value>,
    pub columns: Vec<ColumnMeta>,
}

/// Resolves `target` and applies the request's filters and dedupe. Sorts on unknown columns are
/// dropped; the rest only choose which row a dedupe keeps, as the View's page does.
pub fn filtered(conn: &Connection, target: &Target, request: &QueryRequest) -> ApiResult<Relation> {
    let resolved = resolve_query(conn, target, !request.dedupe_columns.is_empty())?;
    let known_sorts = QueryRequest {
        sorts: request
            .sorts
            .iter()
            .filter(|sort| resolved.columns.iter().any(|meta| meta.name == sort.column))
            .cloned()
            .collect(),
        ..request.clone()
    };
    let built = build_with_rowid(
        &resolved.table,
        &resolved.display_table,
        &resolved.columns,
        &known_sorts,
        resolved.rowid_table.as_deref(),
    )?;
    let mut params = resolved.lead;
    params.extend(built.params);
    Ok(Relation {
        sql: built.relation,
        params,
        columns: resolved.columns,
    })
}

#[cfg(test)]
mod tests;
