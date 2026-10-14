//! The bar and pie charts: the top categories of one column, counted or aggregated over a measure.

use duckdb::Connection;
use serde_json::{Value, json};

use super::model::{ChartKind, ChartSpec, Metric, metric_sql};
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::rows::json_rows;
use crate::runtime::state::invalid_value;
use crate::shared::sql::quote_ident;
use crate::shared::types::profile_kind;

const BAR_LIMIT: usize = 30;
const PIE_LIMIT: usize = 8;

/// The chart's `{chart, rows: [{label, value, n}], series, other_count}`; `other_count` is the
/// non-null rows that fall outside the shown categories.
pub(super) fn chart(conn: &Connection, relation: &Relation, spec: &ChartSpec) -> ApiResult<Value> {
    let is_pie = spec.chart == ChartKind::Pie;
    let (label, name, limit) = if is_pie {
        ("Pie", "pie", PIE_LIMIT)
    } else {
        ("Bar", "bar", BAR_LIMIT)
    };
    let category = spec
        .encodings
        .category
        .as_deref()
        .filter(|name| has_column(relation, name))
        .ok_or_else(|| ApiError::unprocessable(format!("{label} requires a category encoding")))?;
    let field = quote_ident(category);
    let (value_sql, extra) = measure(relation, spec, label)?;
    // The window total rides on every row, so the non-null count needs no second scan.
    let sql = format!(
        "SELECT label, value, n, (sum(n) OVER ())::BIGINT AS non_null FROM (\
         SELECT {field} AS label, {value_sql} AS value, count(*) AS n \
         FROM {} WHERE {field} IS NOT NULL{extra} GROUP BY 1) \
         ORDER BY value DESC NULLS LAST, label LIMIT {limit}",
        relation.sql
    );
    let (_, table) = json_rows(conn, &sql, &relation.params).map_err(invalid_value)?;
    let non_null = table.first().and_then(|row| row[3].as_i64()).unwrap_or(0);
    let shown: i64 = table.iter().filter_map(|row| row[2].as_i64()).sum();
    // Series are C4's; the group column is checked here, after the rows ran, as Python does.
    let group = spec
        .encodings
        .group
        .as_deref()
        .filter(|name| !is_pie && !name.is_empty());
    if group.is_some_and(|name| !has_column(relation, name)) {
        return Err(ApiError::unprocessable(format!(
            "{label} group column was not found"
        )));
    }
    if is_pie && table.iter().any(|row| is_negative(&row[1])) {
        return Err(ApiError::unprocessable(
            "A pie chart cannot show negative values; use a bar chart instead",
        ));
    }
    let rows: Vec<Value> = table
        .into_iter()
        .map(|row| json!({"label": row[0], "value": row[1], "n": row[2]}))
        .collect();
    Ok(json!({
        "chart": name,
        "rows": rows,
        "series": null,
        "other_count": non_null - shown,
    }))
}

fn has_column(relation: &Relation, name: &str) -> bool {
    relation.columns.iter().any(|column| column.name == name)
}

/// The aggregate and the extra `WHERE` terms its measure needs: `count(*)` with no measure,
/// otherwise the metric over a measure with no nulls (and no NaN or infinity unless it only counts).
fn measure(relation: &Relation, spec: &ChartSpec, label: &str) -> ApiResult<(String, String)> {
    let Some(name) = spec
        .encodings
        .value
        .as_deref()
        .filter(|name| !name.is_empty())
    else {
        return Ok(("count(*)".to_owned(), String::new()));
    };
    let column = relation
        .columns
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| ApiError::unprocessable(format!("{label} measure column was not found")))?;
    let metric = spec.metric.unwrap_or(Metric::Avg);
    if metric != Metric::Count && profile_kind(&column.type_name) != Some("numeric") {
        return Err(ApiError::unprocessable(format!(
            "{label} measure requires a numeric column"
        )));
    }
    let field = quote_ident(name);
    let mut extra = format!(" AND {field} IS NOT NULL");
    if !matches!(metric, Metric::Count | Metric::Distinct) {
        extra.push_str(&format!(" AND isfinite({field}::DOUBLE)"));
    }
    Ok((metric_sql(metric, &field), extra))
}

fn is_negative(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64().is_some_and(|number| number < 0.0),
        Value::String(text) => text.parse::<f64>().is_ok_and(|number| number < 0.0),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
