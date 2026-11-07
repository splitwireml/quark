//! ICU-backed time zone behavior of the bundled DuckDB engine.
// Helpers outside `#[test]` fns are not covered by clippy.toml's allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use duckdb::Connection;

fn query_bool(conn: &Connection, sql: &str) -> bool {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn query_string(conn: &Connection, sql: &str) -> String {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn icu_is_loaded() {
    let conn = Connection::open_in_memory().unwrap();
    assert!(query_bool(
        &conn,
        "SELECT loaded FROM duckdb_extensions() WHERE extension_name = 'icu'"
    ));
}

#[test]
fn session_zone_reads_offsetless_timestamptz() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("SET TimeZone = 'Asia/Dubai'").unwrap();
    assert!(query_bool(
        &conn,
        "SELECT TIMESTAMPTZ '2024-01-01 10:00:00' = TIMESTAMPTZ '2024-01-01 06:00:00+00'"
    ));
}

#[test]
fn date_trunc_and_strftime_use_the_session_zone() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("SET TimeZone = 'America/New_York'")
        .unwrap();
    assert_eq!(
        query_string(
            &conn,
            "SELECT strftime(date_trunc('day', TIMESTAMPTZ '2024-03-10 03:30:00+00'), '%Y-%m-%d')"
        ),
        "2024-03-09"
    );
    assert_eq!(
        query_string(
            &conn,
            "SELECT strftime(TIMESTAMPTZ '2024-01-01 02:00:00+00', '%Y')"
        ),
        "2023"
    );
}

#[test]
fn default_zone_is_set() {
    let conn = Connection::open_in_memory().unwrap();
    assert!(!query_string(&conn, "SELECT current_setting('TimeZone')").is_empty());
}

#[test]
fn default_zone_matches_the_machine() {
    let conn = Connection::open_in_memory().unwrap();
    let seconds_east: i64 = conn
        .query_row(
            "SELECT EXTRACT(timezone FROM current_timestamp)::BIGINT",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let machine = chrono::Local::now().offset().local_minus_utc();
    assert_eq!(seconds_east, i64::from(machine));
}
