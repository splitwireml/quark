//! Accepts exactly one read-only SELECT, judged by DuckDB's own parser.

use duckdb::Connection;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::sql::sql_string;

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
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER)").unwrap();
        conn
    }

    #[test]
    fn guard_matches_python_cases() {
        let conn = conn();
        let accepted = [
            "select 1",
            "select 1;",
            "FROM t",
            "with x as (select 1) select * from x",
            "describe t",
            "summarize t",
            "show tables",
            "values (1)",
            "(select 1) union (select 2)",
            "-- c\nselect 1",
            "SELECT 1 AS value; -- trailing",
            "SELECT 1 AS value /* trailing */",
        ];
        for sql in accepted {
            assert!(check_select(&conn, sql).is_ok(), "should accept {sql:?}");
        }
        let rejected = [
            "",
            "   \n\t",
            "select 1; select 2",
            "drop table t",
            "select 1; drop table t",
            "PRAGMA version",
            "explain select 1",
            "pivot t on a",
            "ATTACH '/tmp/items.duckdb'",
            "COPY t TO '/tmp/items.csv'",
            "set threads=1",
            "call pragma_version()",
            "UPDATE t SET a = 1",
            "INSERT INTO t VALUES (1)",
        ];
        for sql in rejected {
            let error = check_select(&conn, sql).unwrap_err();
            assert_eq!(error.status(), 422, "status for {sql:?}");
            assert_eq!(error.detail(), SELECT_ONLY, "detail for {sql:?}");
        }
    }

    #[test]
    fn parse_errors_say_parser_error() {
        let error = check_select(&conn(), "SELEC 1").unwrap_err();
        assert_eq!(error.status(), 422);
        assert!(
            error
                .detail()
                .starts_with("Invalid SQL query: Parser Error: "),
            "got {:?}",
            error.detail()
        );
    }

    #[test]
    fn returns_trimmed_sql() {
        assert_eq!(check_select(&conn(), "  select 1;\n").unwrap(), "select 1;");
    }
}
