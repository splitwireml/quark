//! Unit tests for dataset listing and relation description.

use super::*;

#[test]
fn describe_binds_params_and_reports_types() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (a INTEGER, b VARCHAR)")
        .unwrap();

    let columns = describe(&conn, "t WHERE a > ?", &[&1_i64]).unwrap();

    let pairs: Vec<_> = columns
        .iter()
        .map(|c| (c.name.as_str(), c.type_name.as_str()))
        .collect();
    assert_eq!(pairs, [("a", "INTEGER"), ("b", "VARCHAR")]);
}
