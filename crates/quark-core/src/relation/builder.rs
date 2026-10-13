//! Builds the filtered, deduplicated and ordered SQL for a query request.

use serde_json::Value;
use std::collections::HashSet;

use super::request::{Connector, Direction, Filter, QueryRequest};
use crate::error::{ApiError, ApiResult};
use crate::shared::columns::ColumnMeta;
use crate::shared::literal::literal;
use crate::shared::sql::quote_ident;
use crate::shared::types::is_text;

/// The SQL for one request: a filtered relation, its ordered form, the bound
/// parameters, and a display form with the parameters inlined as literals.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltQuery {
    pub relation: String,
    pub ordered: String,
    pub params: Vec<Value>,
    pub display: String,
}

const OPERATORS: [&str; 13] = [
    "=",
    "!=",
    "in",
    "is_null",
    "not_null",
    "contains",
    "starts_with",
    "ends_with",
    ">",
    ">=",
    "<",
    "<=",
    "between",
];

/// Stringifies a value the way Python's `str()` does for the JSON kinds a filter can carry.
fn python_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        other => other.to_string(),
    }
}

fn is_bound(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_))
}

/// Validates one filter and returns its clause and display clause, pushing its parameters.
fn clause(
    filter: &Filter,
    columns: &[ColumnMeta],
    params: &mut Vec<Value>,
) -> ApiResult<(String, String)> {
    let operator = filter.operator.as_str();
    let meta = columns.iter().find(|meta| meta.name == filter.column);
    let Some(meta) = meta.filter(|_| OPERATORS.contains(&operator)) else {
        return Err(ApiError::unprocessable("Invalid filter column or operator"));
    };
    let column = quote_ident(&meta.name);
    if matches!(operator, "is_null" | "not_null") {
        let not = if operator == "not_null" { "NOT " } else { "" };
        let sql = format!("{column} IS {not}NULL");
        return Ok((sql.clone(), sql));
    }
    let value = filter
        .value
        .as_ref()
        .ok_or_else(|| ApiError::unprocessable("Filter value is required"))?;
    match operator {
        "in" => {
            let items = value.as_array().filter(|items| !items.is_empty());
            let items = items
                .ok_or_else(|| ApiError::unprocessable("IN filter requires a non-empty list"))?;
            let marks = vec!["?"; items.len()].join(", ");
            let shown: Vec<String> = items.iter().map(literal).collect();
            params.extend(items.iter().cloned());
            Ok((
                format!("{column} IN ({marks})"),
                format!("{column} IN ({})", shown.join(", ")),
            ))
        }
        "between" => match value.as_array().map(Vec::as_slice) {
            Some([low, high]) if is_bound(low) && is_bound(high) => {
                params.extend([low.clone(), high.clone()]);
                Ok((
                    format!("{column} BETWEEN ? AND ?"),
                    format!("{column} BETWEEN {} AND {}", literal(low), literal(high)),
                ))
            }
            _ => Err(ApiError::unprocessable(
                "BETWEEN filter requires two bounds",
            )),
        },
        "contains" | "starts_with" | "ends_with" => {
            if !is_text(&meta.type_name) {
                return Err(ApiError::unprocessable(
                    "Text operator requires a text column",
                ));
            }
            let text = Value::String(python_str(value));
            let shown = literal(&text);
            params.push(text);
            Ok((
                format!("{operator}({column}, ?)"),
                format!("{operator}({column}, {shown})"),
            ))
        }
        _ => {
            params.push(value.clone());
            Ok((
                format!("{column} {operator} ?"),
                format!("{column} {operator} {}", literal(value)),
            ))
        }
    }
}

/// Builds the filtered, deduplicated and sorted SQL for `request` over `table`.
/// `display_table` stands in for `table` in the display form.
pub fn build_query(
    table: &str,
    display_table: &str,
    columns: &[ColumnMeta],
    request: &QueryRequest,
) -> ApiResult<BuiltQuery> {
    let mut params = Vec::new();
    let mut folded: Option<(String, String)> = None;
    for filter in &request.filters {
        let (sql, shown) = clause(filter, columns, &mut params)?;
        folded = Some(match folded {
            None => (sql, shown),
            Some((so_far, shown_so_far)) => {
                let connector = match filter.connector {
                    Connector::And => "AND",
                    Connector::Or => "OR",
                };
                (
                    format!("({so_far} {connector} {sql})"),
                    format!("({shown_so_far} {connector} {shown})"),
                )
            }
        });
    }

    let mut seen = HashSet::new();
    let is_valid_dedupe = request
        .dedupe_columns
        .iter()
        .all(|key| columns.iter().any(|meta| meta.name == *key) && seen.insert(key.as_str()));
    if !is_valid_dedupe {
        return Err(ApiError::unprocessable("Invalid dedupe column"));
    }
    let qualify = if request.dedupe_columns.is_empty() {
        String::new()
    } else {
        let keys: Vec<String> = request
            .dedupe_columns
            .iter()
            .map(|key| quote_ident(key))
            .collect();
        format!(
            " QUALIFY row_number() OVER (PARTITION BY {}) = 1",
            keys.join(", ")
        )
    };
    let (where_sql, where_shown) = match folded {
        Some((sql, shown)) => (format!(" WHERE {sql}"), format!(" WHERE {shown}")),
        None => (String::new(), String::new()),
    };
    let relation = format!("(SELECT * FROM {table}{where_sql}{qualify})");
    let display_relation = format!("(SELECT * FROM {display_table}{where_shown}{qualify})");

    let mut orders = Vec::with_capacity(request.sorts.len());
    for sort in &request.sorts {
        if !columns.iter().any(|meta| meta.name == sort.column) {
            return Err(ApiError::unprocessable("Invalid sort column"));
        }
        let direction = match sort.direction {
            Direction::Asc => "ASC",
            Direction::Desc => "DESC",
        };
        orders.push(format!("{} {direction}", quote_ident(&sort.column)));
    }
    let order = if orders.is_empty() {
        String::new()
    } else {
        format!(" ORDER BY {}", orders.join(", "))
    };
    Ok(BuiltQuery {
        ordered: format!("SELECT * FROM {relation}{order}"),
        display: format!("SELECT * FROM {display_relation}{order}"),
        relation,
        params,
    })
}

#[cfg(test)]
mod tests;
