//! Category values for a text column of a dataset or SQL result, through the response cache.

use duckdb::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use super::queries::{count_distinct, fetch_values};
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::filtered;
use crate::relation::request::QueryRequest;
use crate::relation::target::Target;
use crate::runtime::engine::Engine;
use crate::shared::types::is_text;

const MAX_LIMIT: i64 = 500;

fn default_limit() -> i64 {
    200
}

/// Query parameters of the values routes.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ValuesPage {
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

impl ValuesPage {
    /// Rejects a negative `offset` and a `limit` outside `1..=500` with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        if self.offset < 0 || !(1..=MAX_LIMIT).contains(&self.limit) {
            return Err(ApiError::unprocessable("Invalid offset or limit"));
        }
        Ok(())
    }
}

/// The distinct values of `column` containing `page.search`, most frequent first.
/// `column` must be a text column of `target`.
pub fn category_values(
    engine: &Engine,
    conn: &Connection,
    target: &Target,
    column: &str,
    page: &ValuesPage,
) -> ApiResult<Value> {
    page.validate()?;
    let relation = filtered(conn, target, &QueryRequest::default())?;
    let meta = relation
        .columns
        .iter()
        .find(|meta| meta.name == column)
        .ok_or_else(|| ApiError::not_found("Column not found"))?;
    if !is_text(&meta.type_name) {
        return Err(ApiError::unprocessable("Column is not text"));
    }
    let detail = json!({"column": column, "search": page.search, "offset": page.offset, "limit": page.limit});
    engine.cached_response(conn, "values", &relation, &detail, || {
        let unprocessable = |error: duckdb::Error| {
            ApiError::unprocessable(format!("Invalid filter value: {error}"))
        };
        let fetched = fetch_values(
            conn,
            &relation,
            column,
            &page.search,
            page.limit,
            page.offset,
        )
        .map_err(unprocessable)?;
        let total = match fetched.total {
            Some(total) => total,
            None => count_distinct(conn, &relation, column, &page.search).map_err(unprocessable)?,
        };
        let shown = i64::try_from(fetched.rows.len()).unwrap_or(i64::MAX);
        let values: Vec<Value> = fetched
            .rows
            .into_iter()
            .map(|(value, count)| json!({"value": value, "count": count}))
            .collect();
        Ok(json!({
            "values": values,
            "total": total,
            "offset": page.offset,
            "limit": page.limit,
            "has_more": page.offset.saturating_add(shown) < total,
        }))
    })
}
