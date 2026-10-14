//! The line chart: one query buckets the filtered relation by date, ranks the top groups and keeps each series' latest points.

use chrono::Local;
use duckdb::Connection;
use duckdb::arrow::array::{Array, AsArray};
use serde_json::{Value, json};

use super::model::{ChartSpec, Grain, Metric, metric_sql};
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::relation::rows::json_rows;
use crate::runtime::state::invalid_value;
use crate::shared::encode::cell::{cell_json, python_str};
use crate::shared::sql::quote_ident;
use crate::shared::types::{is_timetz, profile_kind};

const TIMETZ: &str = "TIME WITH TIME ZONE";
const SERIES_LIMIT: u32 = 6;
const LINE_POINT_LIMIT: f64 = 400.0;
/// Grains from finest to coarsest, with Python's seconds per grain.
const GRAINS: [(Grain, &str, f64); 6] = [
    (Grain::Hour, "hour", 3600.0),
    (Grain::Day, "day", 86400.0),
    (Grain::Week, "week", 604800.0),
    (Grain::Month, "month", 2629746.0),
    (Grain::Quarter, "quarter", 7889238.0),
    (Grain::Year, "year", 31556952.0),
];

/// The chart's `{chart, grain, series: [{label, points: [{x, y}]}]}`: top groups by row count in
/// rank order, then "Other"; without a group, one series labelled "".
pub(super) fn chart(conn: &Connection, relation: &Relation, spec: &ChartSpec) -> ApiResult<Value> {
    let columns = &relation.columns;
    let type_of = |name: &str| {
        columns
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.type_name.as_str())
    };
    let x_name = spec.encodings.x.as_deref().filter(|name| !name.is_empty());
    let Some(x_type) = x_name.and_then(type_of) else {
        return Err(ApiError::unprocessable("Line requires an x encoding"));
    };
    let x_name = x_name.unwrap_or_default();
    if profile_kind(x_type) != Some("date") {
        return Err(ApiError::unprocessable(format!(
            "Line requires a date or timestamp x column; {x_name} is not one"
        )));
    }
    let x_field = quote_ident(x_name);
    let measure = spec.encodings.y.as_deref().filter(|name| !name.is_empty());
    let metric = spec.metric.unwrap_or(if measure.is_some() {
        Metric::Avg
    } else {
        Metric::Count
    });
    let counts = matches!(metric, Metric::Count | Metric::Distinct);
    let mut filter = format!("{x_field} IS NOT NULL");
    let (value_sql, measure_column) = match measure {
        None => ("count(*)".to_owned(), String::new()),
        Some(name) => {
            let Some(measure_type) = type_of(name) else {
                return Err(ApiError::unprocessable("Line measure column was not found"));
            };
            if !counts && profile_kind(measure_type) != Some("numeric") {
                return Err(ApiError::unprocessable(
                    "Line measure requires a numeric column",
                ));
            }
            let field = quote_ident(name);
            filter.push_str(&format!(" AND {field} IS NOT NULL"));
            if !counts {
                filter.push_str(&format!(" AND isfinite({field}::DOUBLE)"));
            }
            (metric_sql(metric, "s.m"), format!(", {field} AS m"))
        }
    };
    let group = spec
        .encodings
        .group
        .as_deref()
        .filter(|name| !name.is_empty());
    let group_type = group.map(type_of);
    if group_type == Some(None) {
        return Err(ApiError::unprocessable("Line group column was not found"));
    }
    // A TIMETZ is read as text because Arrow drops its offset.
    let label_is_timetz = group_type.flatten().is_some_and(is_timetz);
    let label_sql = if label_is_timetz {
        "CAST(t.series AS VARCHAR)"
    } else {
        "t.series"
    };
    let series_sql = group.map_or_else(|| "''".to_owned(), quote_ident);
    let grain = match spec.grain {
        Some(grain) => name_of(grain),
        None => automatic_grain(conn, relation, &x_field, &filter)?,
    };
    let sql = format!(
        "WITH s AS (SELECT date_trunc('{grain}', {x_field}) AS bucket, {series_sql} AS series{measure_column} \
             FROM {} WHERE {filter}), \
         t AS (SELECT series, row_number() OVER (ORDER BY count(*) DESC, series) AS place \
             FROM s WHERE series IS NOT NULL GROUP BY series QUALIFY place <= {SERIES_LIMIT}) \
         SELECT s.bucket, t.place, {label_sql} AS label, {value_sql} AS value \
         FROM s LEFT JOIN t ON s.series = t.series WHERE s.series IS NOT NULL \
         GROUP BY s.bucket, t.place, t.series \
         QUALIFY row_number() OVER (PARTITION BY t.place IS NOT NULL, t.series ORDER BY s.bucket DESC) <= {} \
         ORDER BY t.place NULLS LAST, s.bucket",
        relation.sql, LINE_POINT_LIMIT as u32,
    );
    let rows = read_points(conn, &sql, &relation.params, label_is_timetz).map_err(invalid_value)?;
    let mut series: Vec<(Option<i64>, Value, Vec<Value>)> = Vec::new();
    for Point {
        bucket,
        place,
        label,
        value,
    } in rows
    {
        if series.last().is_none_or(|(last, ..)| *last != place) {
            series.push((
                place,
                if place.is_some() {
                    json!(label)
                } else {
                    json!("Other")
                },
                Vec::new(),
            ));
        }
        if let Some((.., points)) = series.last_mut() {
            points.push(json!({"x": bucket, "y": value}));
        }
    }
    let mut series: Vec<Value> = series
        .into_iter()
        .map(|(_, label, points)| json!({"label": label, "points": points}))
        .collect();
    if group.is_none() && series.is_empty() {
        series.push(json!({"label": "", "points": []}));
    }
    Ok(json!({"chart": "line", "grain": grain, "series": series}))
}

/// One row of the series query; `place` is `None` for the "Other" bucket.
struct Point {
    bucket: Value,
    place: Option<i64>,
    label: String,
    value: Value,
}

/// Runs the series query: bucket and value as `json_rows` shows them, the label as Python `str()`.
fn read_points(
    conn: &Connection,
    sql: &str,
    params: &[Value],
    label_is_timetz: bool,
) -> duckdb::Result<Vec<Point>> {
    let mut describe = conn.prepare(&format!("DESCRIBE SELECT * FROM ({sql})"))?;
    let types = describe
        .query_map(bind(params), |row| row.get::<_, String>(1))?
        .collect::<duckdb::Result<Vec<_>>>()?;
    let mut statement = conn.prepare(sql)?;
    let mut points = Vec::new();
    for batch in statement.query_arrow(bind(params))? {
        let place = batch
            .column(1)
            .as_primitive::<duckdb::arrow::datatypes::Int64Type>();
        let (buckets, labels, values) = (batch.column(0), batch.column(2), batch.column(3));
        for row in 0..batch.num_rows() {
            let label = if labels.is_null(row) {
                String::new()
            } else if label_is_timetz {
                match cell_json(labels.as_ref(), row, TIMETZ, &Local) {
                    Value::String(text) => text,
                    other => other.to_string(),
                }
            } else {
                python_str(labels.as_ref(), row, &Local)
            };
            points.push(Point {
                bucket: cell_json(buckets.as_ref(), row, &types[0], &Local),
                place: (!place.is_null(row)).then(|| place.value(row)),
                label,
                value: cell_json(values.as_ref(), row, &types[3], &Local),
            });
        }
    }
    Ok(points)
}

fn name_of(grain: Grain) -> &'static str {
    GRAINS
        .iter()
        .find(|(known, ..)| *known == grain)
        .map_or("year", |(_, name, _)| name)
}

/// The finest grain whose bucket count over the data's span fits the point budget, else `year`.
fn automatic_grain(
    conn: &Connection,
    relation: &Relation,
    x_field: &str,
    filter: &str,
) -> ApiResult<&'static str> {
    let sql = format!(
        "SELECT date_diff('second', min({x_field}), max({x_field})) FROM {} WHERE {filter}",
        relation.sql
    );
    let (_, rows) = json_rows(conn, &sql, &relation.params).map_err(invalid_value)?;
    let span = rows.first().and_then(|row| row[0].as_f64()).unwrap_or(0.0);
    Ok(GRAINS
        .iter()
        .find(|(.., seconds)| span / seconds <= LINE_POINT_LIMIT)
        .map_or("year", |(_, name, _)| name))
}

#[cfg(test)]
mod tests;
