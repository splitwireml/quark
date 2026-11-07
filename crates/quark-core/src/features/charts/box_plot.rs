//! The box plot: quartiles, Tukey whiskers and outliers per group, computed in one pass.

use duckdb::Connection;
use serde_json::{Value, json};

use super::model::ChartSpec;
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::rows::json_rows;
use crate::runtime::state::invalid_value;
use crate::shared::sql::quote_ident;
use crate::shared::types::profile_kind;

const BOX_GROUP_LIMIT: usize = 12;
const OUTLIER_LIMIT: usize = 200;

/// The box `{chart, groups}`; one group per label, largest first, "all" without a group.
pub(super) fn chart(conn: &Connection, relation: &Relation, spec: &ChartSpec) -> ApiResult<Value> {
    let has_column = |name: &str| relation.columns.iter().any(|column| column.name == name);
    let Some(value) = spec
        .encodings
        .value
        .as_deref()
        .filter(|name| has_column(name))
    else {
        return Err(ApiError::unprocessable("Box requires a value encoding"));
    };
    let is_numeric = relation
        .columns
        .iter()
        .any(|column| column.name == value && profile_kind(&column.type_name) == Some("numeric"));
    if !is_numeric {
        return Err(ApiError::unprocessable("Box requires a numeric column"));
    }
    let group = spec
        .encodings
        .group
        .as_deref()
        .filter(|name| !name.is_empty());
    if group.is_some_and(|name| !has_column(name)) {
        return Err(ApiError::unprocessable("Box group column was not found"));
    }
    let sql = box_sql(relation, value, group);
    let (_, rows) = json_rows(conn, &sql, &relation.params).map_err(invalid_value)?;
    let groups: Vec<Value> = rows.into_iter().map(group_json).collect();
    Ok(json!({"chart": "box", "groups": groups}))
}

/// Ranks the labels, keeps the largest, then summarizes each label against its own fences.
fn box_sql(relation: &Relation, value: &str, group: Option<&str>) -> String {
    let field = quote_ident(value);
    let finite = format!("{field} IS NOT NULL AND isfinite({field}::DOUBLE)");
    let (label, label_filter) = match group {
        Some(name) => {
            let grouped = quote_ident(name);
            (grouped.clone(), format!("{grouped} IS NOT NULL AND "))
        }
        None => ("'all'".to_owned(), String::new()),
    };
    format!(
        "WITH base AS (SELECT {label} AS label, {field} AS val FROM {source} WHERE {label_filter}{finite}), \
         top AS (SELECT label FROM base GROUP BY label ORDER BY count(*) DESC, label LIMIT {BOX_GROUP_LIMIT}), \
         kept AS (SELECT label, val FROM base WHERE label IN (SELECT label FROM top)), \
         summary AS (SELECT label, quantile_cont(val, 0.25) AS p25, quantile_cont(val, 0.5) AS median, \
                            quantile_cont(val, 0.75) AS p75, count(*) AS n FROM kept GROUP BY label) \
         SELECT s.label, s.p25, s.median, s.p75, \
                min(k.val) FILTER (WHERE k.val >= s.p25 - 1.5 * (s.p75 - s.p25)) AS whisker_low, \
                max(k.val) FILTER (WHERE k.val <= s.p75 + 1.5 * (s.p75 - s.p25)) AS whisker_high, \
                list_slice(coalesce(list(k.val ORDER BY k.val) FILTER (WHERE k.val < s.p25 - 1.5 * (s.p75 - s.p25) \
                    OR k.val > s.p75 + 1.5 * (s.p75 - s.p25)), []), 1, {OUTLIER_LIMIT}) AS outliers, \
                s.n AS count \
         FROM kept k JOIN summary s USING (label) \
         GROUP BY s.label, s.p25, s.median, s.p75, s.n \
         ORDER BY s.n DESC, s.label",
        source = relation.sql
    )
}

fn group_json(cells: Vec<Value>) -> Value {
    let mut cells = cells.into_iter();
    let mut next = || cells.next().unwrap_or(Value::Null);
    json!({
        "label": next(),
        "p25": next(),
        "median": next(),
        "p75": next(),
        "whisker_low": next(),
        "whisker_high": next(),
        "outliers": next(),
        "count": next(),
    })
}

#[cfg(test)]
mod tests;
