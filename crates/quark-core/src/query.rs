//! Request types shared by the data endpoints, and column metadata for responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::error::{ApiError, ApiResult};
use crate::literal::literal;
use crate::sql::quote_ident;
use crate::values::is_text;

const MAX_PAGE_SIZE: i64 = 1000;

/// A column's name and DuckDB type, as reported in response metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Connector {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub column: String,
    pub operator: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub connector: Connector,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub column: String,
    pub direction: Direction,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    100
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl Default for QueryRequest {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
            filters: Vec::new(),
            sorts: Vec::new(),
            dedupe_columns: Vec::new(),
        }
    }
}

impl QueryRequest {
    /// Rejects `page < 1` and `page_size` outside `1..=1000` with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        if self.page < 1 || !(1..=MAX_PAGE_SIZE).contains(&self.page_size) {
            return Err(ApiError::unprocessable("Invalid page or page size"));
        }
        Ok(())
    }
}

/// A [`QueryRequest`] plus the SQL text. The fields are repeated because serde
/// cannot combine `flatten` with `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlQueryRequest {
    pub sql: String,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl SqlQueryRequest {
    pub fn query(&self) -> QueryRequest {
        QueryRequest {
            page: self.page,
            page_size: self.page_size,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe_columns.clone(),
        }
    }
}

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
mod tests {
    use super::*;
    use serde_json::json;

    fn query(value: serde_json::Value) -> Result<QueryRequest, serde_json::Error> {
        serde_json::from_value(value)
    }

    #[test]
    fn defaults_apply() {
        let request = query(json!({})).unwrap();
        assert_eq!(request.page, 1);
        assert_eq!(request.page_size, 100);
        assert!(request.filters.is_empty());
        assert!(request.sorts.is_empty());
        assert!(request.dedupe_columns.is_empty());

        let filter: Filter =
            serde_json::from_value(json!({"column": "a", "operator": "="})).unwrap();
        assert_eq!(filter.value, None);
        assert_eq!(filter.connector, Connector::And);
        let null: Filter =
            serde_json::from_value(json!({"column": "a", "operator": "=", "value": null})).unwrap();
        assert_eq!(null.value, None);

        let sql: SqlQueryRequest = serde_json::from_value(json!({"sql": "SELECT 1"})).unwrap();
        assert_eq!(sql.query(), request);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(query(json!({"page": 1, "extra": true})).is_err());
        let sql = json!({"sql": "SELECT 1", "extra": true});
        assert!(serde_json::from_value::<SqlQueryRequest>(sql).is_err());
    }

    #[test]
    fn paging_bounds() {
        let at = |page, page_size| QueryRequest {
            page,
            page_size,
            ..QueryRequest::default()
        };
        assert!(at(1, 1).validate().is_ok());
        assert!(at(1, 1000).validate().is_ok());
        for bad in [at(0, 100), at(1, 0), at(1, 1001)] {
            let error = bad.validate().unwrap_err();
            assert_eq!(error.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    fn columns() -> Vec<ColumnMeta> {
        [
            ("price", "DOUBLE"),
            ("name", "VARCHAR"),
            ("a", "INTEGER"),
            ("b", "INTEGER"),
            ("c", "INTEGER"),
        ]
        .map(|(name, type_name)| ColumnMeta {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
        })
        .to_vec()
    }

    fn filter(column: &str, operator: &str, value: Option<Value>, connector: Connector) -> Filter {
        Filter {
            column: column.to_owned(),
            operator: operator.to_owned(),
            value,
            connector,
        }
    }

    fn and(column: &str, operator: &str, value: Value) -> Filter {
        filter(column, operator, Some(value), Connector::And)
    }

    fn built(request: &QueryRequest) -> BuiltQuery {
        build_query("\"main\".\"x\"", "\"shown\"", &columns(), request).unwrap()
    }

    fn rejected(request: &QueryRequest) -> String {
        let error = build_query("\"main\".\"x\"", "\"shown\"", &columns(), request).unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
        error.detail().to_owned()
    }

    fn with_filters(filters: Vec<Filter>) -> QueryRequest {
        QueryRequest {
            filters,
            ..QueryRequest::default()
        }
    }

    #[test]
    fn builder_without_filters() {
        let query = built(&QueryRequest::default());
        assert_eq!(query.relation, "(SELECT * FROM \"main\".\"x\")");
        assert_eq!(
            query.ordered,
            "SELECT * FROM (SELECT * FROM \"main\".\"x\")"
        );
        assert_eq!(query.display, "SELECT * FROM (SELECT * FROM \"shown\")");
        assert!(query.params.is_empty());
    }

    #[test]
    fn builder_or_connector() {
        let query = built(&with_filters(vec![
            and("price", ">=", json!(100)),
            filter("name", "contains", Some(json!("a")), Connector::Or),
        ]));
        assert_eq!(
            query.relation,
            "(SELECT * FROM \"main\".\"x\" WHERE (\"price\" >= ? OR contains(\"name\", ?)))"
        );
        assert_eq!(query.params, [json!(100), json!("a")]);
        assert_eq!(
            query.display,
            "SELECT * FROM (SELECT * FROM \"shown\" WHERE (\"price\" >= 100 OR contains(\"name\", 'a')))"
        );
    }

    #[test]
    fn builder_folds_left() {
        let query = built(&with_filters(vec![
            and("a", "=", json!(1)),
            and("b", "=", json!(2)),
            filter("c", "=", Some(json!(3)), Connector::Or),
        ]));
        assert_eq!(
            query.relation,
            "(SELECT * FROM \"main\".\"x\" WHERE ((\"a\" = ? AND \"b\" = ?) OR \"c\" = ?))"
        );
        assert!(
            query
                .display
                .ends_with("WHERE ((\"a\" = 1 AND \"b\" = 2) OR \"c\" = 3))")
        );
    }

    #[test]
    fn builder_in_between_dedupe_sorts() {
        let request = QueryRequest {
            filters: vec![
                and("a", "in", json!([1, 2, 3])),
                and("price", "between", json!([1.5, "z"])),
                filter("b", "is_null", None, Connector::And),
                filter("c", "not_null", None, Connector::And),
                and("name", "starts_with", json!(true)),
                and("name", "ends_with", json!(2.5)),
            ],
            sorts: vec![
                Sort {
                    column: "price".to_owned(),
                    direction: Direction::Asc,
                },
                Sort {
                    column: "name".to_owned(),
                    direction: Direction::Desc,
                },
            ],
            dedupe_columns: vec!["name".to_owned(), "a".to_owned()],
            ..QueryRequest::default()
        };
        let query = built(&request);
        let qualify = " QUALIFY row_number() OVER (PARTITION BY \"name\", \"a\") = 1";
        let order = " ORDER BY \"price\" ASC, \"name\" DESC";
        assert_eq!(
            query.relation,
            format!(
                "(SELECT * FROM \"main\".\"x\" WHERE (((((\"a\" IN (?, ?, ?) AND \"price\" BETWEEN ? AND ?) \
                 AND \"b\" IS NULL) AND \"c\" IS NOT NULL) AND starts_with(\"name\", ?)) \
                 AND ends_with(\"name\", ?)){qualify})"
            )
        );
        assert_eq!(
            query.params,
            [
                json!(1),
                json!(2),
                json!(3),
                json!(1.5),
                json!("z"),
                json!("True"),
                json!("2.5")
            ]
        );
        assert_eq!(
            query.display,
            format!(
                "SELECT * FROM (SELECT * FROM \"shown\" WHERE (((((\"a\" IN (1, 2, 3) AND \"price\" BETWEEN 1.5 AND 'z') \
                 AND \"b\" IS NULL) AND \"c\" IS NOT NULL) AND starts_with(\"name\", 'True')) \
                 AND ends_with(\"name\", '2.5')){qualify}){order}"
            )
        );
        assert_eq!(
            query.ordered,
            format!("SELECT * FROM {}{order}", query.relation)
        );
    }

    #[test]
    fn builder_validation_messages() {
        let one = |f| rejected(&with_filters(vec![f]));
        let invalid = "Invalid filter column or operator";
        assert_eq!(one(and("missing", "=", json!(1))), invalid);
        assert_eq!(one(filter("a", "like", None, Connector::And)), invalid);
        assert_eq!(
            one(filter("a", "=", None, Connector::And)),
            "Filter value is required"
        );
        assert_eq!(
            one(filter("a", "in", None, Connector::And)),
            "Filter value is required"
        );
        let in_message = "IN filter requires a non-empty list";
        assert_eq!(one(and("a", "in", json!([]))), in_message);
        assert_eq!(one(and("a", "in", json!(1))), in_message);
        let between = "BETWEEN filter requires two bounds";
        assert_eq!(one(and("a", "between", json!([1]))), between);
        assert_eq!(one(and("a", "between", json!([1, 2, 3]))), between);
        assert_eq!(one(and("a", "between", json!([1, null]))), between);
        assert_eq!(one(and("a", "between", json!([1, [2]]))), between);
        assert_eq!(one(and("a", "between", json!(1))), between);
        assert_eq!(
            one(and("a", "contains", json!("x"))),
            "Text operator requires a text column"
        );

        let dedupe = |keys: &[&str]| QueryRequest {
            dedupe_columns: keys.iter().map(|key| (*key).to_owned()).collect(),
            ..QueryRequest::default()
        };
        assert_eq!(rejected(&dedupe(&["a", "a"])), "Invalid dedupe column");
        assert_eq!(rejected(&dedupe(&["missing"])), "Invalid dedupe column");

        let sorted = QueryRequest {
            sorts: vec![Sort {
                column: "missing".to_owned(),
                direction: Direction::Asc,
            }],
            ..QueryRequest::default()
        };
        assert_eq!(rejected(&sorted), "Invalid sort column");
    }

    #[test]
    fn connector_and_direction_reject_unknown_words() {
        let connector = |word| {
            serde_json::from_value::<Filter>(
                json!({"column": "a", "operator": "=", "connector": word}),
            )
        };
        assert_eq!(connector("or").unwrap().connector, Connector::Or);
        assert!(connector("xor").is_err());
        assert!(connector("AND").is_err());

        let sort = |word| serde_json::from_value::<Sort>(json!({"column": "a", "direction": word}));
        assert_eq!(sort("desc").unwrap().direction, Direction::Desc);
        assert!(sort("up").is_err());
    }
}
