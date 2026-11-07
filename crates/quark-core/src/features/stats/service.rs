//! Profiles one column of a filtered relation, through the engine's response cache.

use duckdb::Connection;
use serde_json::{Value, json};

use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::filtered;
use crate::relation::profile::{categorical, date, numeric};
use crate::relation::request::QueryRequest;
use crate::relation::target::Target;
use crate::runtime::engine::Engine;
use crate::shared::types::profile_kind;

/// The profile of `column` over `target` with the request's filters and dedupe applied.
/// Dataset and filter errors come before a missing column, as in Python.
pub fn stats(
    engine: &Engine,
    conn: &Connection,
    target: &Target,
    request: &QueryRequest,
    column: &str,
) -> ApiResult<Value> {
    let relation = filtered(conn, target, request)?;
    let meta = relation
        .columns
        .iter()
        .find(|meta| meta.name == column)
        .ok_or_else(|| ApiError::not_found("Column not found"))?;
    let profile = match profile_kind(&meta.type_name) {
        Some("categorical") => categorical::profile,
        Some("date") => date::profile,
        Some(_) => numeric::profile,
        None => return Err(ApiError::unprocessable("Column cannot be profiled")),
    };
    engine.cached_response(conn, "stats", &relation, &json!(column), || {
        profile(conn, &relation, meta)
            .map_err(|error| ApiError::unprocessable(format!("Invalid filter value: {error}")))
    })
}
