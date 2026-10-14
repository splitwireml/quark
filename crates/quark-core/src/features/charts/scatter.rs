//! The scatter chart: a repeatable 5000-point sample, with scales and labels read from the whole relation.

use chrono::Local;
use duckdb::Connection;
use duckdb::arrow::array::Array;
use serde_json::{Map, Value, json};

use super::model::ChartSpec;
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::relation::rows::json_rows;
use crate::runtime::state::invalid_value;
use crate::shared::columns::ColumnMeta;
use crate::shared::encode::cell::{cell_json, python_str};
use crate::shared::sql::quote_ident;
use crate::shared::types::is_timetz;
use columns::{field, find, is_numeric, role, usable, usable_value};

mod columns;

const SCATTER_LIMIT: usize = 5000;
const SERIES_LIMIT: usize = 6;
const OTHER: &str = "Other";

/// One plotted column: its alias in the point, its metadata and, when categorical, its labels.
type Channel<'a> = (&'a str, Option<&'a ColumnMeta>, Option<&'a [String]>);

/// The `{chart, points, total_points, ...}` response; the scale and label keys appear with their encodings.
pub(super) fn chart(conn: &Connection, relation: &Relation, spec: &ChartSpec) -> ApiResult<Value> {
    let encodings = &spec.encodings;
    let (Some(x), Some(y)) = (
        find(relation, encodings.x.as_deref()),
        find(relation, encodings.y.as_deref()),
    ) else {
        return Err(ApiError::unprocessable(
            "Scatter requires x and y encodings",
        ));
    };
    let color = role(relation, "color", encodings.color.as_deref())?;
    let size = role(relation, "size", encodings.size.as_deref())?;
    let shape = role(relation, "shape", encodings.pattern.as_deref())?;
    if size.is_some_and(|meta| !is_numeric(meta)) {
        return Err(ApiError::unprocessable(
            "Scatter size requires a numeric column",
        ));
    }
    let query = Query {
        conn,
        from: format!("{} WHERE {} AND {}", relation.sql, usable(x), usable(y)),
        params: &relation.params,
    };
    let total_points = query.json(&format!("SELECT count(*) FROM {}", query.from))?;

    let color_numeric = color.is_some_and(is_numeric);
    let color_labels = match color {
        Some(meta) if !color_numeric => Some(query.top_labels(meta)?),
        _ => None,
    };
    let shape_labels = shape.map(|meta| query.top_labels(meta)).transpose()?;
    let extras = [
        ("color", color, color_labels.as_deref()),
        ("size", size, None),
        ("shape", shape, shape_labels.as_deref()),
    ];
    let points = query.points(x, y, &extras)?;

    let mut response = json!({
        "chart": "scatter",
        "points": points,
        "total_points": total_points,
    });
    if let Some(meta) = color {
        response["color_kind"] = json!(if color_numeric {
            "numeric"
        } else {
            "categorical"
        });
        if color_numeric {
            response["color_domain"] = query.domain(meta)?;
        } else {
            response["color_labels"] = with_other(color_labels.unwrap_or_default());
        }
    }
    if let Some(meta) = size {
        response["size_domain"] = query.domain(meta)?;
    }
    if shape.is_some() {
        response["shape_labels"] = with_other(shape_labels.unwrap_or_default());
    }
    Ok(response)
}

/// The filtered relation narrowed to plottable rows, with the parameters its SQL binds.
struct Query<'a> {
    conn: &'a Connection,
    from: String,
    params: &'a [Value],
}

impl Query<'_> {
    /// The first cell of the first row of a one-row aggregate.
    fn json(&self, sql: &str) -> ApiResult<Value> {
        let (_, rows) = json_rows(self.conn, sql, self.params).map_err(invalid_value)?;
        Ok(rows.into_iter().flatten().next().unwrap_or(Value::Null))
    }

    /// `[min, max]` over the finite values of the whole relation, never over the sample.
    fn domain(&self, meta: &ColumnMeta) -> ApiResult<Value> {
        let field = quote_ident(&meta.name);
        let sql = format!(
            "SELECT min({field}), max({field}) FROM {} AND {}",
            self.from,
            usable_value(&field)
        );
        let (_, rows) = json_rows(self.conn, &sql, self.params).map_err(invalid_value)?;
        Ok(Value::Array(rows.into_iter().next().unwrap_or_default()))
    }

    /// The six most common values by count, ties by value, rendered as Python `str()` does.
    fn top_labels(&self, meta: &ColumnMeta) -> ApiResult<Vec<String>> {
        let raw = quote_ident(&meta.name);
        let shown = if is_timetz(&meta.type_name) {
            format!("min(CAST({raw} AS VARCHAR))")
        } else {
            raw.clone()
        };
        let sql = format!(
            "SELECT {shown} FROM {} AND {raw} IS NOT NULL GROUP BY {raw} \
             ORDER BY count(*) DESC, {raw} LIMIT {SERIES_LIMIT}",
            self.from
        );
        let mut statement = self.conn.prepare(&sql).map_err(invalid_value)?;
        let batches = statement
            .query_arrow(bind(self.params))
            .map_err(invalid_value)?;
        Ok(batches
            .flat_map(|batch| {
                let column = batch.column(0).clone();
                (0..column.len())
                    .map(|row| label(column.as_ref(), row, meta))
                    .collect::<Vec<_>>()
            })
            .collect())
    }

    /// Up to 5000 points, sampled from the plottable rows. The sample is seeded, so the same View and data draw the same points.
    fn points(&self, x: &ColumnMeta, y: &ColumnMeta, extras: &[Channel]) -> ApiResult<Vec<Value>> {
        let columns: Vec<(&str, &ColumnMeta, Option<&[String]>)> =
            [("x", Some(x), None), ("y", Some(y), None)]
                .into_iter()
                .chain(extras.iter().copied())
                .filter_map(|(alias, meta, labels)| meta.map(|meta| (alias, meta, labels)))
                .collect();
        let select: Vec<String> = columns
            .iter()
            .map(|(alias, meta, _)| format!("{} AS {alias}", field(meta)))
            .collect();
        let sql = format!(
            "SELECT * FROM (SELECT {} FROM {}) USING SAMPLE reservoir({SCATTER_LIMIT} ROWS) REPEATABLE (42)",
            select.join(", "),
            self.from
        );
        let mut statement = self.conn.prepare(&sql).map_err(invalid_value)?;
        let batches = statement
            .query_arrow(bind(self.params))
            .map_err(invalid_value)?;
        let mut points = Vec::new();
        for batch in batches {
            for row in 0..batch.num_rows() {
                let mut record = Map::new();
                for ((alias, meta, labels), array) in columns.iter().zip(batch.columns()) {
                    let value = match labels {
                        Some(labels) if !array.is_null(row) => {
                            fold(label(array.as_ref(), row, meta), labels)
                        }
                        _ => cell_json(array.as_ref(), row, &meta.type_name, &Local),
                    };
                    record.insert((*alias).to_owned(), value);
                }
                points.push(Value::Object(record));
            }
        }
        Ok(points)
    }
}

/// A categorical value as Python `str()` shows it; a TIMETZ read as text gets its `isoformat()` offset.
fn label(array: &dyn Array, row: usize, meta: &ColumnMeta) -> String {
    if is_timetz(&meta.type_name) {
        match cell_json(array, row, &meta.type_name, &Local) {
            Value::String(text) => text,
            other => other.to_string(),
        }
    } else {
        python_str(array, row, &Local)
    }
}

/// A label inside the top six stays; any other value becomes `"Other"`.
fn fold(text: String, labels: &[String]) -> Value {
    Value::String(if labels.contains(&text) {
        text
    } else {
        OTHER.to_owned()
    })
}

/// A list that filled all six slots gets `"Other"` for what did not fit.
fn with_other(mut labels: Vec<String>) -> Value {
    if labels.len() == SERIES_LIMIT {
        labels.push(OTHER.to_owned());
    }
    json!(labels)
}

#[cfg(test)]
mod tests;
