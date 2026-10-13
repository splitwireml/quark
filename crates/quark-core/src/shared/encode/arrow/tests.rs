//! Unit tests for Arrow stream encoding of query results.

use super::*;
use arrow::array::{Array, AsArray};
use arrow::datatypes::{DataType, Int32Type};
use arrow::ipc::reader::StreamReader;
use duckdb::Connection;
use serde_json::json;
use std::io::Cursor;

fn meta() -> Map<String, Value> {
    json!({"page": 1, "total_rows": 3})
        .as_object()
        .cloned()
        .unwrap()
}

/// Column names and types of `sql`, as DuckDB describes them.
fn describe_columns(conn: &Connection, sql: &str) -> Vec<ColumnMeta> {
    let mut statement = conn.prepare(&format!("DESCRIBE {sql}")).unwrap();
    statement
        .query_map([], |row| {
            Ok(ColumnMeta {
                name: row.get(0)?,
                type_name: row.get(1)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Runs `sql` and encodes the result the way `run_page` does.
fn encode(sql: &str, keep_batches: bool) -> (Vec<RecordBatch>, Vec<u8>) {
    let conn = Connection::open_in_memory().unwrap();
    let columns = describe_columns(&conn, sql);
    let mut statement = conn.prepare(sql).unwrap();
    let arrow = statement.query_arrow([]).unwrap();
    let schema = arrow.get_schema();
    let batches: Vec<RecordBatch> = arrow.collect();
    let used = if keep_batches { &batches[..] } else { &[] };
    let bytes = encode_page(&schema, used, &columns, meta()).unwrap();
    (batches, bytes)
}

fn decode(bytes: Vec<u8>) -> (Schema, Vec<RecordBatch>) {
    let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
    let schema = reader.schema().as_ref().clone();
    (schema, reader.map(Result::unwrap).collect())
}

#[test]
fn arrow_page_round_trips_with_metadata() {
    let (original, bytes) = encode(
        "SELECT range::INTEGER AS a, 'x' || range AS b, range % 2 = 0 AS c FROM range(3)",
        true,
    );

    let (schema, batches) = decode(bytes);

    let meta: Value = serde_json::from_str(&schema.metadata()["quark"]).unwrap();
    assert_eq!(
        meta,
        json!({"page": 1, "total_rows": 3, "json_columns": []})
    );
    assert_eq!(batches.len(), original.len());
    assert_eq!(batches[0].columns(), original[0].columns());
}

#[test]
fn mixed_page_keeps_native_columns_native() {
    let (original, bytes) = encode(
        "SELECT 7::INTEGER AS a, DATE '2024-01-02' AS d, NULL::DATE AS n, 1.50::DECIMAL(5,2) AS m",
        true,
    );

    let (schema, batches) = decode(bytes);

    let meta: Value = serde_json::from_str(&schema.metadata()["quark"]).unwrap();
    assert_eq!(meta["json_columns"], json!(["d", "n", "m"]));
    let types: Vec<&DataType> = schema.fields().iter().map(|f| f.data_type()).collect();
    assert_eq!(
        types,
        [
            &DataType::Int32,
            &DataType::Utf8,
            &DataType::Utf8,
            &DataType::Utf8
        ]
    );
    let batch = &batches[0];
    assert_eq!(batch.column(0), original[0].column(0));
    assert_eq!(batch.column(0).as_primitive::<Int32Type>().value(0), 7);
    assert_eq!(
        batch.column(1).as_string::<i32>().value(0),
        "\"2024-01-02\""
    );
    assert!(!batch.column(2).is_null(0));
    assert_eq!(batch.column(2).as_string::<i32>().value(0), "null");
    assert_eq!(batch.column(3).as_string::<i32>().value(0), "1.5");
}

#[test]
fn empty_page_is_schema_only() {
    let (_, bytes) = encode(
        "SELECT 1::INTEGER AS a, DATE '2024-01-02' AS d WHERE false",
        false,
    );

    let (schema, batches) = decode(bytes);

    assert!(batches.is_empty());
    let types: Vec<&DataType> = schema.fields().iter().map(|f| f.data_type()).collect();
    assert_eq!(types, [&DataType::Int32, &DataType::Utf8]);
    let meta: Value = serde_json::from_str(&schema.metadata()["quark"]).unwrap();
    assert_eq!(meta["json_columns"], json!(["d"]));
}
