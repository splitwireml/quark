//! The metric card: one aggregate over the filtered relation, from a column and an operation or from the user's own formula.

use duckdb::Connection;
use serde_json::{Value, json};

use super::model::{ChartSpec, Metric, metric_sql};
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::guard::check_select;
use crate::relation::rows::json_rows;
use crate::runtime::state::invalid_value;
use crate::shared::sql::quote_ident;
use crate::shared::types::profile_kind;

/// The card's `{chart, value, rows}`; `rows` is how many rows the aggregate covered.
pub(super) fn chart(conn: &Connection, relation: &Relation, spec: &ChartSpec) -> ApiResult<Value> {
    let expression = spec.expression.as_deref().map_or("", str::trim);
    let value_sql = if expression.is_empty() {
        column_sql(relation, spec)?
    } else {
        formula_sql(conn, expression)?
    };
    let sql = format!(
        "SELECT {value_sql} AS value, count(*) AS rows FROM {}",
        relation.sql
    );
    let (_, rows) = json_rows(conn, &sql, &relation.params).map_err(|error| {
        if expression.is_empty() {
            invalid_value(error)
        } else {
            ApiError::unprocessable(format!("This metric formula did not run: {error}"))
        }
    })?;
    let mut cells = rows.into_iter().next().unwrap_or_default().into_iter();
    Ok(json!({
        "chart": "metric",
        "value": cells.next().unwrap_or(Value::Null),
        "rows": cells.next().unwrap_or(Value::Null),
    }))
}

/// The user's expression, held to the SQL editor's single read-only SELECT rule.
/// The guard parses `SELECT <expression> AS value FROM quark_relation`: the relation's own `?`
/// placeholders cannot be serialized, and the guard only reads the statement's shape.
fn formula_sql(conn: &Connection, expression: &str) -> ApiResult<String> {
    if expression.contains('?') {
        return Err(ApiError::unprocessable(
            "A metric formula cannot contain ? placeholders",
        ));
    }
    check_select(
        conn,
        &format!("SELECT {expression} AS value FROM quark_relation"),
    )?;
    Ok(expression.to_owned())
}

/// `count(*)` with no value column; otherwise the operation over that column.
fn column_sql(relation: &Relation, spec: &ChartSpec) -> ApiResult<String> {
    let Some(name) = spec.encodings.value.as_deref() else {
        return Ok("count(*)".to_owned());
    };
    let column = relation
        .columns
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| ApiError::unprocessable("Metric column was not found"))?;
    let metric = spec.metric.unwrap_or(Metric::Count);
    let needs_numbers = matches!(
        metric,
        Metric::Sum | Metric::Avg | Metric::Median | Metric::Stddev
    );
    if needs_numbers && profile_kind(&column.type_name) != Some("numeric") {
        return Err(ApiError::unprocessable(
            "This metric requires a numeric column",
        ));
    }
    Ok(metric_sql(metric, &quote_ident(name)))
}
