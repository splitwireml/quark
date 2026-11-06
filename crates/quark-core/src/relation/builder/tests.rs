//! Unit tests for building filtered, sorted relations.

use super::*;
use crate::relation::request::Sort;
use serde_json::json;

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
    // The dedupe and its order are pinned by `deduped_sql_orders_by_the_sorts_then_one_position`.
    assert!(query.relation.contains(
        "(((((\"a\" IN (?, ?, ?) AND \"price\" BETWEEN ? AND ?) AND \"b\" IS NULL) \
         AND \"c\" IS NOT NULL) AND starts_with(\"name\", ?)) AND ends_with(\"name\", ?))"
    ));
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
    assert!(query.display.contains(
        "(((((\"a\" IN (1, 2, 3) AND \"price\" BETWEEN 1.5 AND 'z') AND \"b\" IS NULL) \
         AND \"c\" IS NOT NULL) AND starts_with(\"name\", 'True')) AND ends_with(\"name\", '2.5'))"
    ));
    assert!(!query.display.contains('?'));
}

/// One `ORDER BY` term: its column (empty for any other expression) and whether it descends.
type Term = (String, bool);

/// The terms of every `ORDER BY` inside a dedupe window (one with a `PARTITION BY`) in `sql`, and
/// of the statement's own `ORDER BY`.
fn order_terms(sql: &str) -> (Vec<Vec<Term>>, Vec<Term>) {
    fn terms(orders: &Value) -> Vec<Term> {
        orders
            .as_array()
            .unwrap()
            .iter()
            .map(|order| {
                let expression = &order["expression"];
                let column = match expression["class"].as_str() {
                    Some("COLUMN_REF") => expression["column_names"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    _ => String::new(),
                };
                (column, order["type"] == "DESCENDING")
            })
            .collect()
    }
    fn windows(node: &Value, found: &mut Vec<Vec<Term>>) {
        match node {
            Value::Object(map) => {
                let is_dedupe = map.get("class").is_some_and(|class| class == "WINDOW")
                    && map["partitions"]
                        .as_array()
                        .is_some_and(|keys| !keys.is_empty());
                if is_dedupe {
                    found.push(terms(&map["orders"]));
                }
                map.values().for_each(|value| windows(value, found));
            }
            Value::Array(items) => items.iter().for_each(|value| windows(value, found)),
            _ => {}
        }
    }
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let text: String = conn
        .query_row("SELECT json_serialize_sql(?::VARCHAR)", [sql], |row| {
            row.get(0)
        })
        .unwrap();
    let tree: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(tree["error"], false, "{sql}: {tree}");
    let mut found = Vec::new();
    windows(&tree, &mut found);
    let statement = &tree["statements"][0]["node"];
    let outer = statement["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|modifier| modifier["type"] == "ORDER_MODIFIER")
        .map_or_else(Vec::new, |modifier| terms(&modifier["orders"]));
    (found, outer)
}

#[test]
fn deduped_sql_orders_by_the_sorts_then_one_position() {
    // The View's sorts, then one position term that is none of the View's columns: the same
    // number of terms whatever the View's width, so the cost does not grow with it.
    for width in [3, 60] {
        let columns: Vec<ColumnMeta> = (0..width)
            .map(|index| ColumnMeta {
                name: format!("c{index}"),
                type_name: if index % 2 == 0 { "VARCHAR" } else { "BIGINT" }.to_owned(),
            })
            .collect();
        let request = QueryRequest {
            sorts: vec![
                Sort {
                    column: "c1".to_owned(),
                    direction: Direction::Desc,
                },
                Sort {
                    column: "c2".to_owned(),
                    direction: Direction::Asc,
                },
            ],
            dedupe_columns: vec!["c0".to_owned()],
            ..QueryRequest::default()
        };
        let query = build_query("\"main\".\"x\"", "\"shown\"", &columns, &request).unwrap();
        let sorts = [("c1".to_owned(), true), ("c2".to_owned(), false)];
        let is_view_column = |name: &str| columns.iter().any(|meta| meta.name == name);
        let check = |terms: &[Term], what: &str| {
            assert_eq!(terms.len(), 3, "{width} columns, {what}: {terms:?}");
            assert_eq!(terms[..2], sorts, "{width} columns, {what}");
            assert!(
                !is_view_column(&terms[2].0),
                "{width} columns, {what}: {terms:?}"
            );
        };
        for (sql, name) in [
            (&query.relation, "relation"),
            (&query.ordered, "ordered"),
            (&query.display, "display"),
        ] {
            let sql = if name == "relation" {
                format!("SELECT * FROM {sql}")
            } else {
                sql.clone()
            };
            let (windows, outer) = order_terms(&sql);
            assert!(
                !windows.is_empty(),
                "{width} columns, {name}: no dedupe window"
            );
            for window in &windows {
                check(window, &format!("{name} window"));
            }
            if name != "relation" {
                check(&outer, &format!("{name} ORDER BY"));
            }
        }
    }
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
