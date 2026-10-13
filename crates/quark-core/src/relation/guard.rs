//! Accepts exactly one read-only SELECT, judged by DuckDB's own parser.

use duckdb::Connection;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::shared::sql::sql_string;

const SELECT_ONLY: &str = "SQL accepts only one read-only SELECT query";

/// Returns the trimmed SQL when it is a single SELECT-shaped statement.
pub fn check_select(conn: &Connection, sql: &str) -> ApiResult<String> {
    // json_serialize_sql only folds a constant argument, so a bound `?` is rejected.
    let json: String = conn
        .query_row(
            &format!("SELECT json_serialize_sql({})", sql_string(sql)),
            [],
            |row| row.get(0),
        )
        .map_err(|_| select_only())?;
    let parsed: Value = serde_json::from_str(&json).map_err(|_| select_only())?;
    if parsed["error"] == Value::Bool(true) {
        return Err(
            match (
                parsed["error_type"].as_str(),
                parsed["error_message"].as_str(),
            ) {
                (Some("parser"), Some(message)) => {
                    ApiError::unprocessable(format!("Invalid SQL query: Parser Error: {message}"))
                }
                _ => select_only(),
            },
        );
    }
    match parsed["statements"].as_array() {
        Some(statements) if statements.len() == 1 => Ok(sql.trim().to_owned()),
        _ => Err(select_only()),
    }
}

fn select_only() -> ApiError {
    ApiError::unprocessable(SELECT_ONLY)
}

#[cfg(test)]
mod tests;
