//! Tests for the typed workbook cache and for mounting it into nodes and workspaces.

use super::*;
use crate::relation::describe::datasets;
use crate::runtime::dirs::Dirs;
use crate::runtime::engine::Engine;
use crate::runtime::registry::{ProjectRecord, SourceRecord};
use duckdb::{AccessMode, Config};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, Worksheet};

fn strings(sheet: &mut Worksheet, column: u16, values: &[&str]) {
    for (row, value) in (0u32..).zip(values) {
        sheet.write_string(row, column, *value).unwrap();
    }
}

fn numbers(sheet: &mut Worksheet, column: u16, header: &str, values: &[f64]) {
    sheet.write_string(0, column, header).unwrap();
    for (row, value) in (1u32..).zip(values) {
        sheet.write_number(row, column, *value).unwrap();
    }
}

/// Saves one sheet named `Sheet` into `dir` as `book.xlsx`.
fn save(dir: &Path, fill: impl FnOnce(&mut Worksheet)) -> PathBuf {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Sheet").unwrap();
    fill(sheet);
    let path = dir.join("book.xlsx");
    workbook.save(&path).unwrap();
    path
}

fn sheets() -> Vec<String> {
    vec!["Sheet".to_owned()]
}

fn open_cache(file: &Path) -> Connection {
    let config = Config::default().access_mode(AccessMode::ReadOnly).unwrap();
    Connection::open_with_flags(file, config).unwrap()
}

/// One cell of the cached sheet as text, by column and row position.
fn cell(conn: &Connection, table: &str, column: &str, row: usize) -> String {
    let sql = format!(
        "SELECT CAST({} AS VARCHAR) FROM {} OFFSET {row} LIMIT 1",
        quote_ident(column),
        quote_ident(table)
    );
    conn.query_row(&sql, [], |found| found.get(0)).unwrap()
}

fn column_type(conn: &Connection, table: &str, column: &str) -> String {
    conn.query_row(
        "SELECT data_type FROM duckdb_columns() WHERE table_name = ? AND column_name = ?",
        [table, column],
        |row| row.get(0),
    )
    .unwrap()
}

fn dirs_in(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

fn record(file: &Path, id: &str, sheets: Option<Vec<String>>) -> SourceRecord {
    SourceRecord {
        id: id.to_owned(),
        name: id.to_owned(),
        kind: "upload".to_owned(),
        source: file.to_path_buf(),
        project_id: Some("p1".to_owned()),
        dataset_name: None,
        sheets,
    }
}

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "p1".to_owned(),
        name: "P1".to_owned(),
        node_id: "project_p1".to_owned(),
    }
}

fn cache_files(cache: &Path) -> Vec<PathBuf> {
    fs::read_dir(cache.join("columnar"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default()
}

fn one_value(engine: &Engine, sql: &str) -> i64 {
    engine
        .primary()
        .conn
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

#[test]
fn typed_tables_follow_python_rules() {
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| {
        strings(sheet, 0, &["ids", "007", "12"]);
        strings(
            sheet,
            1,
            &["when", "2024-01-02 12:00:00", "2024-01-03 08:30:00"],
        );
        strings(sheet, 2, &["day", "2024-01-02", "2024-01-03"]);
        strings(sheet, 3, &["word", "a", "b"]);
        let mostly = ["mostly", "n/a", "1", "1", "1", "1", "1", "1", "1", "1", "1"];
        strings(sheet, 4, &mostly);
        let mut few = vec!["few"];
        few.extend(["1", "2", "3", "4", "5", "6", "7", "a", "b", "c", "d"]);
        strings(sheet, 5, &few);
        numbers(sheet, 6, "flag", &[1.0, 0.0]);
    });

    let cached = import_workbook(&file, &sheets(), &dir.path().join("cache")).unwrap();

    let conn = open_cache(&cached);
    let types: Vec<_> = ["ids", "when", "day", "word", "mostly", "few", "flag"]
        .iter()
        .map(|column| column_type(&conn, "Sheet", column))
        .collect();
    assert_eq!(
        types,
        [
            "DOUBLE",
            "TIMESTAMP",
            "DATE",
            "VARCHAR",
            "DOUBLE",
            "VARCHAR",
            "DOUBLE"
        ]
    );
    assert_eq!(cell(&conn, "Sheet", "ids", 0), "7.0");
    assert_eq!(cell(&conn, "Sheet", "when", 1), "2024-01-03 08:30:00");
    assert_eq!(cell(&conn, "Sheet", "few", 0), "1");
    let missing: Option<String> = conn
        .query_row(
            "SELECT CAST(mostly AS VARCHAR) FROM \"Sheet\" LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(missing, None);
}

#[test]
fn serial_dates_reinterpret_only_when_declared() {
    let day = Format::new().set_num_format("yyyy-mm-dd");
    let moment = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
    let clock = Format::new().set_num_format("hh:mm:ss");
    let at = |text| ExcelDateTime::parse_from_str(text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| {
        for (column, header) in (0u16..).zip(["day", "late_fraction", "moment", "clock", "general"])
        {
            sheet.write_string(0, column, header).unwrap();
        }
        sheet
            .write_datetime_with_format(1, 0, at("2024-01-02"), &day)
            .unwrap();
        sheet
            .write_datetime_with_format(2, 0, at("2024-01-03"), &day)
            .unwrap();
        sheet
            .write_datetime_with_format(1, 1, at("2024-01-02"), &day)
            .unwrap();
        sheet
            .write_datetime_with_format(2, 1, at("2024-01-03 12:00:00"), &day)
            .unwrap();
        sheet
            .write_datetime_with_format(1, 2, at("2024-01-02 12:00:00"), &moment)
            .unwrap();
        sheet
            .write_datetime_with_format(1, 3, at("12:00:00"), &clock)
            .unwrap();
        sheet.write_number(1, 4, 45293.5).unwrap();
    });

    let cached = import_workbook(&file, &sheets(), &dir.path().join("cache")).unwrap();

    let conn = open_cache(&cached);
    let found = |column: &str| {
        (
            column_type(&conn, "Sheet", column),
            cell(&conn, "Sheet", column, 0),
        )
    };
    let text = |kind: &str, value: &str| (kind.to_owned(), value.to_owned());
    assert_eq!(found("day"), text("DATE", "2024-01-02"));
    assert_eq!(
        found("late_fraction"),
        text("TIMESTAMP", "2024-01-02 00:00:00")
    );
    assert_eq!(found("moment"), text("TIMESTAMP", "2024-01-02 12:00:00"));
    assert_eq!(found("clock"), text("TIME", "12:00:00"));
    assert_eq!(found("general"), text("DOUBLE", "45293.5"));
}

#[test]
fn node_engine_lists_sheets_as_tables() {
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| numbers(sheet, 0, "n", &[1.0, 2.0]));

    let engine = Engine::open_node(&record(&file, "s1", None), &dirs_in(dir.path())).unwrap();

    let listed: Vec<_> = datasets(&engine.primary().conn)
        .unwrap()
        .into_iter()
        .map(|item| (item.schema, item.name, item.kind))
        .collect();
    assert_eq!(
        listed,
        [("main".to_owned(), "Sheet".to_owned(), "TABLE".to_owned())]
    );
    assert_eq!(
        one_value(&engine, "SELECT sum(n)::BIGINT FROM \"Sheet\""),
        3
    );
    assert_eq!(cache_files(&dir.path().join("cache")).len(), 1);
}

#[test]
fn workspace_views_read_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| numbers(sheet, 0, "n", &[1.0, 2.0]));
    let source = record(&file, "s1", Some(sheets()));
    let workspace = Engine::open_workspace(
        &project(),
        std::slice::from_ref(&source),
        &dirs_in(dir.path()),
    )
    .unwrap();

    let views = workspace.mount(&project(), &source).unwrap();

    assert_eq!(views.len(), 1);
    assert_eq!(
        (views[0].name.as_str(), views[0].kind.as_str()),
        ("Sheet", "TABLE")
    );
    assert_eq!(views[0].columns, ["n"]);
    assert_eq!(
        one_value(
            &workspace,
            &format!("SELECT sum(n)::BIGINT FROM ({})", views[0].sql)
        ),
        3
    );
    assert_eq!(cache_files(&dir.path().join("cache")).len(), 1);
}

/// Counterpart of the Python-only `test_project_source_api_layers_mount_only_the_requested_source`.
#[test]
fn workspace_mounts_only_the_requested_source() {
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| numbers(sheet, 0, "n", &[1.0]));
    let csv = dir.path().join("other.csv");
    fs::write(&csv, "a\n1\n").unwrap();
    let book = record(&file, "s1", None);
    let other = record(&csv, "s2", None);
    let sources = [other, book.clone()];
    let workspace = Engine::open_workspace(&project(), &sources, &dirs_in(dir.path())).unwrap();

    workspace.mount(&project(), &book).unwrap();

    let views = one_value(
        &workspace,
        "SELECT count(*) FROM duckdb_views() WHERE schema_name LIKE 'source_%'",
    );
    assert_eq!(views, 1);
}

#[test]
fn evicted_cache_is_imported_again() {
    let dir = tempfile::tempdir().unwrap();
    let file = save(dir.path(), |sheet| numbers(sheet, 0, "n", &[1.0, 2.0]));
    let cache = dir.path().join("cache");
    let source = record(&file, "s1", Some(sheets()));
    let workspace = Engine::open_workspace(
        &project(),
        std::slice::from_ref(&source),
        &dirs_in(dir.path()),
    )
    .unwrap();
    let cached = import_workbook(&file, &sheets(), &cache).unwrap();
    assert!(workbook_key(&file, &sheets()).unwrap().is_ready(&cache));
    fs::remove_file(&cached).unwrap();

    workspace.mount(&project(), &source).unwrap();
    assert!(cached.is_file());
    drop(workspace);
    fs::remove_file(&cached).unwrap();
    Engine::open_node(&source, &dirs_in(dir.path())).unwrap();
    assert!(cached.is_file());
}

#[test]
fn import_leaves_no_partial_file_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("broken.xlsx");
    fs::write(&file, "not a workbook").unwrap();
    let cache = dir.path().join("cache");

    let result = import_workbook(&file, &sheets(), &cache);

    assert!(result.is_err());
    assert!(cache_files(&cache).is_empty());
}
