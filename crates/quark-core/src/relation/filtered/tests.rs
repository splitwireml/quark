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

#[test]
fn dedupe_keeps_the_row_the_sort_puts_first_every_time() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        // `id` counts down, so the first row of a tie has the highest id.
        "CREATE TABLE t AS SELECT 199999 - i AS id, i % 7 AS grp, (i * 7919) % 1000 AS price FROM range(200000) r(i)",
    )
    .unwrap();
    let request = QueryRequest {
        sorts: vec![Sort {
            column: "price".to_owned(),
            direction: Direction::Desc,
        }],
        dedupe_columns: vec!["grp".to_owned()],
        ..QueryRequest::default()
    };
    let relation = filtered(&conn, &dataset(), &request).unwrap();
    let kept = || -> Vec<(i64, i64)> {
        let sql = format!("SELECT grp, id FROM {} ORDER BY grp", relation.sql);
        let mut statement = conn.prepare(&sql).unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };

    // Per group: the highest price, then the first of the rows sharing it.
    let mut statement = conn
        .prepare(
            "SELECT grp, max(id) FROM t WHERE (grp, price) IN \
             (SELECT (grp, max(price)) FROM t GROUP BY grp) GROUP BY grp ORDER BY grp",
        )
        .unwrap();
    let expected: Vec<(i64, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(expected.len(), 7);
    for _ in 0..5 {
        assert_eq!(kept(), expected);
    }
}

#[test]
fn deduped_view_orders_tied_sort_keys_the_same_way_every_time() {
    let conn = Connection::open_in_memory().unwrap();
    // Every group's lowest price is 0, so the View's sort alone leaves the groups tied. `id` counts
    // down, so ordering ties by any column instead of by position would turn the order around.
    conn.execute_batch(
        "CREATE TABLE t AS SELECT 199999 - i AS id, i % 40 AS grp, (i // 40) % 1000 AS price FROM range(200000) r(i)",
    )
    .unwrap();
    let columns = crate::relation::describe::describe(&conn, "t", &[]).unwrap();
    let request = QueryRequest {
        sorts: vec![Sort {
            column: "price".to_owned(),
            direction: Direction::Asc,
        }],
        dedupe_columns: vec!["grp".to_owned()],
        ..QueryRequest::default()
    };
    let built = crate::relation::builder::build_query("t", "t", &columns, &request).unwrap();
    let ids = || -> Vec<i64> {
        let sql = format!("SELECT id FROM ({})", built.ordered);
        let mut statement = conn.prepare(&sql).unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };

    // Each group keeps its first row (rows 0 to 39), and the tied groups keep their source order.
    let expected: Vec<i64> = (0..40).map(|row| 199_999 - row).collect();
    for _ in 0..5 {
        assert_eq!(ids(), expected, "tied prices fall back to source order");
    }
    // Pages, result tables and exports read every column of these, so the position stays hidden.
    for sql in [&built.ordered, &built.display] {
        let mut statement = conn.prepare(&format!("DESCRIBE {sql}")).unwrap();
        let names: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(names, ["id", "grp", "price"], "{sql}");
    }
}
