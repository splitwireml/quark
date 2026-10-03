use duckdb::Connection;

#[test]
fn engine_matches_python_duckdb_version() {
    let lock =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../uv.lock")).unwrap();
    let mut lines = lock.lines();
    lines
        .find(|line| line.trim() == r#"name = "duckdb""#)
        .unwrap();
    let version = lines.find(|line| line.starts_with("version = ")).unwrap();
    let locked = version.trim_start_matches("version = ").trim_matches('"');

    let engine: String = Connection::open_in_memory()
        .unwrap()
        .query_row("SELECT version()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(engine.trim_start_matches('v'), locked);
}
