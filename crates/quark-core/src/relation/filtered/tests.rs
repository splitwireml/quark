//! Unit tests for building filtered relations.

use super::*;
use crate::relation::request::{Direction, Filter, Sort};
use crate::shared::ids::dataset_id;
use serde_json::json;

fn conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t AS SELECT 1 AS a, 'x' AS b")
        .unwrap();
    conn
}

fn dataset() -> Target {
    Target::Dataset(dataset_id("main", "t"))
}

fn equals(column: &str, value: Value) -> Filter {
    Filter {
        column: column.to_owned(),
        operator: "=".to_owned(),
        value: Some(value),
        connector: Default::default(),
    }
}

#[test]
fn dataset_target_has_no_lead_params() {
    let request = QueryRequest {
        filters: vec![equals("a", json!(1))],
        ..QueryRequest::default()
    };

    let relation = filtered(&conn(), &dataset(), &request).unwrap();

    assert_eq!(relation.params, vec![json!(1)]);
    assert_eq!(
        relation.sql,
        "(SELECT * FROM \"main\".\"t\" WHERE \"a\" = ?)"
    );
    assert_eq!(relation.columns.len(), 2);
}

#[test]
fn sql_target_leads_with_its_sql() {
    let request = QueryRequest {
        filters: vec![equals("b", json!("x"))],
        ..QueryRequest::default()
    };
    let target = Target::Sql("select 1 as a, 'x' as b".to_owned());

    let relation = filtered(&conn(), &target, &request).unwrap();

    assert_eq!(
        relation.params,
        vec![json!("select 1 as a, 'x' as b"), json!("x")]
    );
    assert_eq!(relation.sql, "(SELECT * FROM query(?) WHERE \"b\" = ?)");
}

#[test]
fn unknown_sort_column_is_ignored() {
    let request = QueryRequest {
        sorts: vec![Sort {
            column: "missing".to_owned(),
            direction: Direction::Asc,
        }],
        ..QueryRequest::default()
    };

    let relation = filtered(&conn(), &dataset(), &request).unwrap();

    assert_eq!(relation.sql, "(SELECT * FROM \"main\".\"t\")");
}

#[test]
fn duplicate_dedupe_key_is_422() {
    let request = QueryRequest {
        dedupe_columns: vec!["a".to_owned(), "a".to_owned()],
        ..QueryRequest::default()
    };

    let error = filtered(&conn(), &dataset(), &request).unwrap_err();

    assert_eq!(error.status().as_u16(), 422);
    assert_eq!(error.detail(), "Invalid dedupe column");
}
