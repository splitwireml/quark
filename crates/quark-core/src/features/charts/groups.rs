//! The series of a grouped bar chart: the six largest groups and an "Other" bucket, with one value per bar and series.

use std::collections::HashMap;

use duckdb::Connection;
use duckdb::arrow::array::AsArray;
use serde_json::Value;

use crate::relation::filtered::Relation;
use crate::relation::params::bind;
use crate::relation::rows::json_rows;
use crate::shared::columns::ColumnMeta;
use crate::shared::encode::cell::{cell_json, python_str};
use crate::shared::sql::quote_ident;
use crate::shared::types::is_timetz;

use super::bar::NAN_BAR;

const SERIES_LIMIT: usize = 6;

/// What the bars were built from: the quoted category column, the aggregate over the measure and
/// the `WHERE` terms the measure needs.
pub(super) struct Bars<'a> {
    pub field: &'a str,
    pub value_sql: &'a str,
    pub extra: &'a str,
}

/// The series names and, for each bar label (as its `CAST(label AS VARCHAR)` key), its values
/// aligned to them.
pub(super) struct Series {
    pub names: Vec<String>,
    pub values: HashMap<String, Vec<Value>>,
}

/// Ranks the groups by row count, then sums the bars' measure per group; the rest fold into "Other".
/// Bars and groups are matched on their text keys (`CAST(x AS VARCHAR)`), which survive any type a
/// JSON round trip would not (NaN, INTERVAL, TIMETZ). `None` when there are no bars or no group.
pub(super) fn series(
    conn: &Connection,
    relation: &Relation,
    bars: &Bars,
    group: &ColumnMeta,
    label_keys: &[String],
) -> duckdb::Result<Option<Series>> {
    let (top, names) = rank(conn, relation, bars, group)?;
    if label_keys.is_empty() || top.is_empty() {
        return Ok(None);
    }
    let group_field = quote_ident(&group.name);
    let field = bars.field;
    let marks = |count: usize| vec!["?"; count].join(", ");
    let label_marks = marks(label_keys.len());
    let top_marks = marks(top.len());
    let sql = format!(
        "WITH picked AS (SELECT * FROM {} WHERE CAST({field} AS VARCHAR) IN ({label_marks}) \
         AND {group_field} IS NOT NULL{extra}) \
         SELECT CAST({field} AS VARCHAR) AS label_key, \
         CAST({group_field} AS VARCHAR) IN ({top_marks}) AS is_top, \
         CASE WHEN CAST({group_field} AS VARCHAR) IN ({top_marks}) \
         THEN CAST({group_field} AS VARCHAR) END AS series_key, \
         {} AS value FROM picked GROUP BY 1, 2, 3",
        relation.sql,
        bars.value_sql,
        extra = bars.extra,
    );
    let texts = |keys: &[String]| keys.iter().cloned().map(Value::String).collect::<Vec<_>>();
    // The NaN bar is bound as its real cast text so its rows are fetched, and the ones outside the
    // top series still make "Other"; its `values` entry stays under the sentinel, so its cells are 0.
    let fetched: Vec<String> = label_keys
        .iter()
        .map(|key| if key == NAN_BAR { "nan" } else { key }.to_owned())
        .collect();
    let params: Vec<Value> = [
        relation.params.as_slice(),
        &texts(&fetched),
        &texts(&top),
        &texts(&top),
    ]
    .concat();
    let (_, cells) = json_rows(conn, &sql, &params)?;
    let mut names = names;
    let has_other = cells.iter().any(|cell| cell[1] == Value::Bool(false));
    if has_other {
        names.push("Other".to_owned());
    }
    let mut values: HashMap<String, Vec<Value>> = label_keys
        .iter()
        .map(|key| (key.clone(), vec![Value::from(0); names.len()]))
        .collect();
    for cell in cells {
        let slot = if cell[1] == Value::Bool(true) {
            top.iter().position(|key| cell[2].as_str() == Some(key))
        } else {
            Some(names.len() - 1)
        };
        let row = cell[0].as_str().and_then(|key| values.get_mut(key));
        if let (Some(slot), Some(row)) = (slot, row) {
            row[slot] = cell[3].clone();
        }
    }
    Ok(Some(Series { names, values }))
}

/// The groups with the most rows over the bars' source: their text keys and Python `str()` names.
fn rank(
    conn: &Connection,
    relation: &Relation,
    bars: &Bars,
    group: &ColumnMeta,
) -> duckdb::Result<(Vec<String>, Vec<String>)> {
    let group_field = quote_ident(&group.name);
    // A TIMETZ is read as text because Arrow drops its offset.
    let shown = if is_timetz(&group.type_name) {
        "CAST(series AS VARCHAR)"
    } else {
        "series"
    };
    let sql = format!(
        "SELECT CAST(series AS VARCHAR) AS series_key, {shown} AS series_name FROM (\
         SELECT {group_field} AS series, count(*) AS n FROM {} \
         WHERE {} IS NOT NULL AND {group_field} IS NOT NULL{} \
         GROUP BY 1 ORDER BY n DESC, series LIMIT {SERIES_LIMIT}) ORDER BY n DESC, series",
        relation.sql, bars.field, bars.extra
    );
    let mut statement = conn.prepare(&sql)?;
    let (mut top, mut names) = (Vec::new(), Vec::new());
    for batch in statement.query_arrow(bind(&relation.params))? {
        let (keys, series) = (batch.column(0).as_string::<i32>(), batch.column(1).as_ref());
        for row in 0..batch.num_rows() {
            top.push(keys.value(row).to_owned());
            names.push(
                match cell_json(series, row, &group.type_name, &chrono::Local) {
                    Value::String(text) if is_timetz(&group.type_name) => text,
                    _ => python_str(series, row, &chrono::Local),
                },
            );
        }
    }
    Ok((top, names))
}

#[cfg(test)]
mod tests;
