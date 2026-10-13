//! Binds JSON values as DuckDB query parameters.

use duckdb::types::{Null, ToSqlOutput};
use duckdb::{ToSql, params_from_iter};
use serde_json::Value;

/// Binds a JSON value as a DuckDB parameter; arrays and objects bind as their JSON text.
struct Param<'a>(&'a Value);

impl ToSql for Param<'_> {
    fn to_sql(&self) -> duckdb::Result<ToSqlOutput<'_>> {
        Ok(match self.0 {
            Value::Null => Null.into(),
            Value::Bool(flag) => (*flag).into(),
            Value::Number(number) => match (number.as_i64(), number.as_u64()) {
                (Some(signed), _) => signed.into(),
                (None, Some(unsigned)) => unsigned.into(),
                _ => number.as_f64().unwrap_or_default().into(),
            },
            Value::String(text) => ToSqlOutput::from(text.as_str()),
            other => other.to_string().into(),
        })
    }
}

pub(crate) fn bind(params: &[Value]) -> impl duckdb::Params {
    params_from_iter(params.iter().map(Param))
}
