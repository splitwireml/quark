use super::*;

fn conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (a INTEGER)").unwrap();
    conn
}

#[test]
fn guard_matches_python_cases() {
    let conn = conn();
    let accepted = [
        "select 1",
        "select 1;",
        "FROM t",
        "with x as (select 1) select * from x",
        "describe t",
        "summarize t",
        "show tables",
        "values (1)",
        "(select 1) union (select 2)",
        "-- c\nselect 1",
        "SELECT 1 AS value; -- trailing",
        "SELECT 1 AS value /* trailing */",
    ];
    for sql in accepted {
        assert!(check_select(&conn, sql).is_ok(), "should accept {sql:?}");
    }
    let rejected = [
        "",
        "   \n\t",
        "select 1; select 2",
        "drop table t",
        "select 1; drop table t",
        "PRAGMA version",
        "explain select 1",
        "pivot t on a",
        "ATTACH '/tmp/items.duckdb'",
        "COPY t TO '/tmp/items.csv'",
        "set threads=1",
        "call pragma_version()",
        "UPDATE t SET a = 1",
        "INSERT INTO t VALUES (1)",
    ];
    for sql in rejected {
        let error = check_select(&conn, sql).unwrap_err();
        assert_eq!(error.status(), 422, "status for {sql:?}");
        assert_eq!(error.detail(), SELECT_ONLY, "detail for {sql:?}");
    }
}

#[test]
fn parse_errors_say_parser_error() {
    let error = check_select(&conn(), "SELEC 1").unwrap_err();
    assert_eq!(error.status(), 422);
    assert!(
        error
            .detail()
            .starts_with("Invalid SQL query: Parser Error: "),
        "got {:?}",
        error.detail()
    );
}

#[test]
fn returns_trimmed_sql() {
    assert_eq!(check_select(&conn(), "  select 1;\n").unwrap(), "select 1;");
}
