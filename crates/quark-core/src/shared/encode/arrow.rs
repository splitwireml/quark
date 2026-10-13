//! Encodes a page as an Arrow IPC stream: native columns keep their arrays, the rest become JSON text.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use serde_json::{Map, Value};

use super::cell::cell_json;
use crate::error::{ApiError, ApiResult};
use crate::shared::columns::ColumnMeta;
use crate::shared::types::is_native;

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
mod tests;
