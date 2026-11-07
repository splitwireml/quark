//! The histogram: the column's profile bins, plus per-series counts over the same bin edges when a group is given.

use std::collections::HashMap;

use duckdb::Connection;
use serde_json::{Value, json};

use super::groups::{Bars, rank};
use super::model::ChartSpec;
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::relation::profile::{date, numeric};
use crate::runtime::engine::Engine;
use crate::runtime::state::invalid_value;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;
use crate::shared::types::profile_kind;

const UNIFORM_TOLERANCE: f64 = 1e-6;

/// How a row's value picks its bin: the SQL for the bin index and the parameters that precede the relation's.
struct Bucket {
    sql: String,
    params: Vec<Value>,
}

/// The card's `{chart, bins}`, plus `series` when a group is given and the column has bins.
/// The bins are the column's profile bins, shared with the stats panel through the response cache.
pub(super) fn chart(
    engine: &Engine,
    conn: &Connection,
    relation: &Relation,
    spec: &ChartSpec,
) -> ApiResult<Value> {
    let column = spec
        .encodings
        .value
        .as_deref()
        .and_then(|name| relation.columns.iter().find(|column| column.name == name))
        .ok_or_else(|| ApiError::unprocessable("Histogram requires a value encoding"))?;
    let kind = match profile_kind(&column.type_name) {
        Some(kind @ ("numeric" | "date")) => kind,
        _ => {
            return Err(ApiError::unprocessable(
                "Histogram requires a numeric or date column",
            ));
        }
    };
    let profile = engine.cached_response(conn, "stats", relation, &json!(column.name), || {
        let profile = if kind == "date" {
            date::profile(conn, relation, column)
        } else {
            numeric::profile(conn, relation, column)
        };
        profile.map_err(invalid_value)
    })?;
    let bins = profile["histogram"].as_array().cloned().unwrap_or_default();
    let mut response = json!({"chart": "histogram", "bins": bins});
    let group = match spec.encodings.group.as_deref() {
        Some(name) if !name.is_empty() => relation
            .columns
            .iter()
            .find(|column| column.name == name)
            .ok_or_else(|| ApiError::unprocessable("Histogram group column was not found"))?,
        _ => return Ok(response),
    };
    if !bins.is_empty() {
        let series = series(conn, relation, (column, kind), group, &bins).map_err(invalid_value)?;
        if !series.is_empty() {
            response["series"] = Value::Array(series);
        }
    }
    Ok(response)
}

/// The width formula for numeric profiles whose bins are equal-width (within 1e-6 relative),
/// otherwise one `CASE` over the bins' upper bounds.
fn bucket(column: &ColumnMeta, kind: &str, field: &str, bins: &[Value]) -> Bucket {
    if kind == "numeric"
        && let Some(width) = uniform_width(bins)
    {
        return Bucket {
            sql: format!("least(?, floor(({field} - ?) / ?)::INTEGER)"),
            params: vec![
                json!(bins.len() - 1),
                bins[0]["lower"].clone(),
                json!(width),
            ],
        };
    }
    // The first bin whose upper bound is at least the value; the last bin takes the rest.
    let uppers = &bins[..bins.len() - 1];
    if uppers.is_empty() {
        return Bucket {
            sql: "CAST(0 AS INTEGER)".to_owned(),
            params: Vec::new(),
        };
    }
    let arms: String = uppers
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                "WHEN {field} <= CAST(? AS {}) THEN {index} ",
                column.type_name
            )
        })
        .collect();
    Bucket {
        sql: format!("CASE {arms}ELSE {} END", bins.len() - 1),
        params: uppers.iter().map(|bin| bin["upper"].clone()).collect(),
    }
}

/// The common bin width when there are at least two bins and every width matches the first.
fn uniform_width(bins: &[Value]) -> Option<f64> {
    let width_of = |bin: &Value| Some(bin["upper"].as_f64()? - bin["lower"].as_f64()?);
    let first = width_of(bins.first()?)?;
    let all_equal = bins.len() >= 2
        && first > 0.0
        && bins.iter().all(|bin| {
            width_of(bin).is_some_and(|width| (width - first).abs() <= first * UNIFORM_TOLERANCE)
        });
    all_equal.then_some(first)
}

/// Counts per bin for the six largest groups over the finite rows, with every other group folded
/// into `"Other"`. Empty when no row has both a value and a group.
fn series(
    conn: &Connection,
    relation: &Relation,
    (column, kind): (&ColumnMeta, &str),
    group: &ColumnMeta,
    bins: &[Value],
) -> duckdb::Result<Vec<Value>> {
    let field = quote_ident(&column.name);
    let group_field = quote_ident(&group.name);
    let finite = if kind == "numeric" {
        format!("{field} IS NOT NULL AND isfinite({field}::DOUBLE) AND {group_field} IS NOT NULL")
    } else {
        format!("{field} IS NOT NULL AND {group_field} IS NOT NULL")
    };
    let extra = if kind == "numeric" {
        format!(" AND isfinite({field}::DOUBLE)")
    } else {
        String::new()
    };
    // The same ranking a grouped bar chart uses: text keys to match rows, Python `str()` names to show.
    let bars = Bars {
        field: &field,
        value_sql: "",
        extra: &extra,
    };
    let (keys, names) = rank(conn, relation, &bars, group)?;
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let bucket = bucket(column, kind, &field, bins);
    // The bucket's parameters sit in the SELECT, ahead of the relation's in the FROM.
    let counted_sql = format!(
        "SELECT CAST({group_field} AS VARCHAR), {} AS idx, count(*) AS n FROM {} WHERE {finite} \
         GROUP BY {group_field}, idx",
        bucket.sql, relation.sql
    );
    let params: Vec<Value> = bucket
        .params
        .into_iter()
        .chain(relation.params.iter().cloned())
        .collect();
    // Counts per bin for each ranked group, by position in `keys`; the last slot is "Other".
    let mut buckets: Vec<HashMap<i64, i64>> = vec![HashMap::new(); keys.len() + 1];
    let mut statement = conn.prepare(&counted_sql)?;
    let rows = statement.query_map(bind(&params), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (key, index, n) = row?;
        let slot = keys
            .iter()
            .position(|top| *top == key)
            .unwrap_or(keys.len());
        *buckets[slot].entry(index).or_insert(0) += n;
    }
    let mut labels = names;
    if !buckets[keys.len()].is_empty() {
        labels.push("Other".to_owned());
    }
    Ok(labels
        .into_iter()
        .zip(buckets)
        .map(|(label, counts)| {
            let bins: Vec<Value> = bins
                .iter()
                .zip(0_i64..)
                .map(|(bin, index)| {
                    let count = counts.get(&index).copied();
                    json!({"lower": bin["lower"], "upper": bin["upper"], "count": count.unwrap_or(0)})
                })
                .collect();
            json!({"label": label, "bins": bins})
        })
        .collect())
}

#[cfg(test)]
mod tests;
