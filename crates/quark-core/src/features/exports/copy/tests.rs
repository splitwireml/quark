//! Tests for the Parquet and JSON writers: kept types, both JSON layouts, repeated names and empty results.

use std::fs;
use std::path::Path;

use duckdb::Connection;
use serde_json::{Value, json};

use super::super::model::{ExportFormat, ExportSheet, JsonLayout};
use super::super::names::extension;
use super::super::service::{ExportFile, prepare_sheet};
use super::{travels_as_text, write_copy};
use crate::shared::sql::sql_string;

fn sheet(sql: &str) -> ExportSheet {
    ExportSheet {
        node_id: "n".into(),
        name: "Sheet".into(),
        sql: sql.into(),
    }
}

fn source() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE people(id INTEGER, name VARCHAR, score DOUBLE, born DATE, seen TIMESTAMP,
                             price DECIMAL(10,2), active BOOLEAN, tags VARCHAR[], stamp TIMESTAMPTZ);
         INSERT INTO people VALUES
           (1, 'Ada', 1.5, DATE '1815-12-10', TIMESTAMP '2020-01-02 03:04:05', 10.25, true, ['a', 'b'], TIMESTAMPTZ '2020-01-02 03:04:05+00'),
           (2, NULL, NULL, NULL, NULL, NULL, false, [], NULL);",
    )
    .unwrap();
    conn
}

/// Writes `sql` and returns the file's text (JSON) or leaves it for `read_back` (Parquet).
fn export(
    conn: &Connection,
    format: ExportFormat,
    layout: JsonLayout,
    sql: &str,
    dir: &Path,
    read: impl FnOnce(&Path),
) -> crate::error::ApiResult<()> {
    let prepared = prepare_sheet(conn, &sheet(sql))?;
    let (file, handle) = ExportFile::create(dir, extension(format))?;
    drop(handle);
    write_copy(conn, &prepared, format, layout, file.path(), dir)?;
    read(file.path());
    Ok(())
}

fn json_file(conn: &Connection, layout: JsonLayout, sql: &str) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let mut parsed = Value::Null;
    export(conn, ExportFormat::Json, layout, sql, dir.path(), |path| {
        parsed = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    })
    .unwrap();
    parsed
}

fn parquet_types(path: &Path) -> Vec<(String, String)> {
    let conn = Connection::open_in_memory().unwrap();
    let mut statement = conn
        .prepare(&format!(
            "DESCRIBE SELECT * FROM read_parquet({})",
            sql_string(&path.to_string_lossy())
        ))
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn parquet_keeps_column_types() {
    let conn = source();
    let dir = tempfile::tempdir().unwrap();
    let mut types = Vec::new();
    export(
        &conn,
        ExportFormat::Parquet,
        JsonLayout::Rows,
        "SELECT * FROM people ORDER BY id",
        dir.path(),
        |path| types = parquet_types(path),
    )
    .unwrap();
    let expected = [
        ("id", "INTEGER"),
        ("name", "VARCHAR"),
        ("score", "DOUBLE"),
        ("born", "DATE"),
        ("seen", "TIMESTAMP"),
        ("price", "DECIMAL(10,2)"),
        ("active", "BOOLEAN"),
        ("tags", "VARCHAR[]"),
        ("stamp", "TIMESTAMP WITH TIME ZONE"),
    ];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(name, kind)| ((*name).to_owned(), (*kind).to_owned()))
        .collect();
    assert_eq!(types, expected);
}

#[test]
fn json_rows_are_an_array_of_objects() {
    let conn = source();
    let rows = json_file(
        &conn,
        JsonLayout::Rows,
        "SELECT id, name, price FROM people ORDER BY id",
    );
    assert_eq!(
        rows,
        json!([
            {"id": 1, "name": "Ada", "price": 10.25},
            {"id": 2, "name": null, "price": null},
        ])
    );
}

#[test]
fn json_columns_hold_lists_even_when_empty() {
    let conn = source();
    let columns = json_file(
        &conn,
        JsonLayout::Columns,
        "SELECT id, name FROM people ORDER BY id",
    );
    assert_eq!(columns, json!({"id": [1, 2], "name": ["Ada", null]}));
    let none = json_file(
        &conn,
        JsonLayout::Columns,
        "SELECT id, tags FROM people WHERE id > 99",
    );
    assert_eq!(none, json!({"id": [], "tags": []}));
}

#[test]
fn duplicate_names_are_422() {
    let conn = source();
    let dir = tempfile::tempdir().unwrap();
    for (format, label) in [
        (ExportFormat::Parquet, "PARQUET"),
        (ExportFormat::Json, "JSON"),
    ] {
        let error = export(
            &conn,
            format,
            JsonLayout::Columns,
            "SELECT id, id FROM people",
            dir.path(),
            |_| (),
        )
        .unwrap_err();
        assert_eq!(error.status().as_u16(), 422);
        assert_eq!(
            error.detail(),
            format!("{label} export requires unique column names")
        );
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn empty_results_export_valid_files() {
    let conn = source();
    let dir = tempfile::tempdir().unwrap();
    let sql = "SELECT id, name FROM people WHERE id > 99";
    let mut types = Vec::new();
    export(
        &conn,
        ExportFormat::Parquet,
        JsonLayout::Rows,
        sql,
        dir.path(),
        |path| {
            types = parquet_types(path);
        },
    )
    .unwrap();
    assert_eq!(
        types,
        [
            ("id".into(), "INTEGER".into()),
            ("name".into(), "VARCHAR".into())
        ]
    );
    assert_eq!(json_file(&conn, JsonLayout::Rows, sql), json!([]));
}

/// The file's exact text without DuckDB's line breaks: `serde_json` would turn big integers into floats.
fn json_text(conn: &Connection, layout: JsonLayout, sql: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let mut text = String::new();
    export(conn, ExportFormat::Json, layout, sql, dir.path(), |path| {
        text = fs::read_to_string(path).unwrap().replace(['\n', '\t'], "");
    })
    .unwrap();
    text
}

#[test]
fn json_rows_write_wide_integers_bits_and_enums_exactly() {
    let conn = source();
    let cases = [
        (
            "[170141183460469231731687303715884105727::HUGEINT]",
            r#"[{"c":[170141183460469231731687303715884105727]}]"#,
        ),
        ("[1::HUGEINT, 2]", r#"[{"c":[1,2]}]"#),
        ("MAP {'k': 5::HUGEINT}", r#"[{"c":{"k":5}}]"#),
        (
            "340282366920938463463374607431768211455::UHUGEINT",
            r#"[{"c":340282366920938463463374607431768211455}]"#,
        ),
        ("'101'::BIT", r#"[{"c":"101"}]"#),
        ("'a'::ENUM('a', 'b')", r#"[{"c":"a"}]"#),
        ("[1.5::DECIMAL(38,2)]", r#"[{"c":[1.50]}]"#),
        ("NULL::HUGEINT", r#"[{"c":null}]"#),
    ];
    for (expr, expected) in cases {
        let text = json_text(&conn, JsonLayout::Rows, &format!("SELECT {expr} AS c"));
        assert_eq!(text, expected, "{expr}");
    }
}

#[test]
fn json_columns_write_wide_integers_bits_and_enums_exactly() {
    let conn = source();
    let text = json_text(
        &conn,
        JsonLayout::Columns,
        "SELECT [170141183460469231731687303715884105727::HUGEINT] AS h, '101'::BIT AS b,
                'a'::ENUM('a', 'b') AS e,
                340282366920938463463374607431768211455::UHUGEINT AS u",
    );
    assert_eq!(
        text,
        r#"{"h":[[170141183460469231731687303715884105727]],"b":["101"],"e":["a"],"u":[340282366920938463463374607431768211455]}"#
    );
}

#[test]
fn parquet_stores_wide_integers_as_doubles_and_bits_and_enums_as_text() {
    let conn = source();
    let dir = tempfile::tempdir().unwrap();
    let mut types = Vec::new();
    export(
        &conn,
        ExportFormat::Parquet,
        JsonLayout::Rows,
        "SELECT [1::HUGEINT] AS h, 2::UHUGEINT AS u, '101'::BIT AS b, 'a'::ENUM('a', 'b') AS e",
        dir.path(),
        |path| types = parquet_types(path),
    )
    .unwrap();
    let expected = [
        ("h", "DOUBLE[]"),
        ("u", "DOUBLE"),
        ("b", "VARCHAR"),
        ("e", "VARCHAR"),
    ];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(name, kind)| ((*name).to_owned(), (*kind).to_owned()))
        .collect();
    assert_eq!(types, expected);
}

#[test]
fn text_transfer_covers_nested_types_but_not_names() {
    for kind in [
        "HUGEINT",
        "UHUGEINT[]",
        "MAP(VARCHAR, UHUGEINT)",
        "STRUCT(a INTEGER, b HUGEINT[])",
        "BIT",
        "ENUM('a', 'b')[]",
        "VARINT",
        "TIME WITH TIME ZONE[]",
    ] {
        assert!(travels_as_text(kind), "{kind}");
    }
    for kind in [
        "INTEGER",
        "DECIMAL(38,2)[]",
        "TIMESTAMP WITH TIME ZONE",
        "STRUCT(\"bit\" INTEGER, \"hugeint\" VARCHAR)",
    ] {
        assert!(!travels_as_text(kind), "{kind}");
    }
}
