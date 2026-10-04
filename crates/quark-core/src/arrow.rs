//! Encodes a page as an Arrow IPC stream: native columns keep their arrays, the rest become JSON text.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use serde_json::{Map, Value};

use crate::error::{ApiError, ApiResult};
use crate::query::ColumnMeta;
use crate::values::{cell_json, is_native};

pub const ARROW_MEDIA_TYPE: &str = "application/vnd.apache.arrow.stream";

fn internal(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "could not encode an Arrow page");
    ApiError::internal("Internal error")
}

/// One JSON text per cell, `"null"` for nulls.
fn json_array(array: &dyn Array, type_name: &str) -> ArrayRef {
    let mut builder = StringBuilder::with_capacity(array.len(), array.len() * 8);
    for row in 0..array.len() {
        let cell = cell_json(array, row, type_name, &chrono::Local);
        builder.append_value(cell.to_string());
    }
    Arc::new(builder.finish())
}

/// Writes `batches` as an Arrow stream whose schema carries `meta` and `json_columns` under `quark`.
/// `schema` is the statement's, so a page with no batches still has its columns.
pub fn encode_page(
    schema: &Schema,
    batches: &[RecordBatch],
    columns: &[ColumnMeta],
    mut meta: Map<String, Value>,
) -> ApiResult<Vec<u8>> {
    let natives: Vec<bool> = columns
        .iter()
        .map(|column| is_native(&column.type_name))
        .collect();
    let json_columns: Vec<&str> = columns
        .iter()
        .zip(&natives)
        .filter(|&(_, &is_native)| !is_native)
        .map(|(column, _)| column.name.as_str())
        .collect();
    meta.insert("json_columns".to_owned(), Value::from(json_columns));

    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .zip(&natives)
        .map(|(field, &is_native)| {
            if is_native {
                field.as_ref().clone()
            } else {
                Field::new(field.name(), DataType::Utf8, true)
            }
        })
        .collect();
    let metadata = HashMap::from([("quark".to_owned(), Value::Object(meta).to_string())]);
    let encoded = Arc::new(Schema::new_with_metadata(fields, metadata));

    let mut writer = StreamWriter::try_new(Vec::new(), &encoded).map_err(internal)?;
    for batch in batches {
        let arrays = batch
            .columns()
            .iter()
            .zip(columns)
            .zip(&natives)
            .map(|((array, column), &is_native)| {
                if is_native {
                    Arc::clone(array)
                } else {
                    json_array(array.as_ref(), &column.type_name)
                }
            })
            .collect();
        let page = RecordBatch::try_new(Arc::clone(&encoded), arrays).map_err(internal)?;
        writer.write(&page).map_err(internal)?;
    }
    writer.into_inner().map_err(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::describe;
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

    /// Runs `sql` and encodes the result the way `run_page` does.
    fn encode(sql: &str, keep_batches: bool) -> (Vec<RecordBatch>, Vec<u8>) {
        let conn = Connection::open_in_memory().unwrap();
        let columns = describe(&conn, "query(?)", &[&sql]).unwrap();
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
}
