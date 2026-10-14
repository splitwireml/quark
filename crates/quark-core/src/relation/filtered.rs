//! Builds the filtered, deduplicated relation that analysis routes read, with its bound parameters.

use duckdb::Connection;
use serde_json::Value;

use super::builder::build_query;
use super::request::QueryRequest;
use super::target::{Target, resolve};
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

/// Resolves `target` and applies the request's filters and dedupe. Sorts are dropped unvalidated.
pub fn filtered(conn: &Connection, target: &Target, request: &QueryRequest) -> ApiResult<Relation> {
    let resolved = resolve(conn, target)?;
    let unsorted = QueryRequest {
        sorts: Vec::new(),
        ..request.clone()
    };
    let built = build_query(
        &resolved.table,
        &resolved.display_table,
        &resolved.columns,
        &unsorted,
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
