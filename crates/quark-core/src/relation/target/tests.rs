//! Unit tests for resolving a request to its target relation.

use super::*;
use duckdb::Connection;

#[test]
fn parser_errors_while_describing_say_select_only() {
    let conn = Connection::open_in_memory().unwrap();

    let parser = describe(&conn, "query(?)", &[&"selec 1"]).unwrap_err();
    let other = describe(&conn, "query(?)", &[&"select * from missing"]).unwrap_err();

    assert_eq!(
        describe_error(parser).detail(),
        "SQL accepts only one read-only SELECT query"
    );
    assert!(
        describe_error(other)
            .detail()
            .starts_with("Invalid SQL query: ")
    );
}
