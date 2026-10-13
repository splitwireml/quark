//! Converts one Arrow cell, including nested lists, structs and maps, to JSON like the Python backend.

use super::scalar::{
    binary_json, date_json, decimal128_json, decimal256_json, float_json, hex_json, integer_json,
    interval_json, split, string_json, time_json, timestamp_json, value,
};
use crate::shared::literal::python_float_repr;
use chrono::TimeZone;
use duckdb::arrow::array::{Array, AsArray, downcast_dictionary_array};
use duckdb::arrow::datatypes::{
    DataType, Date32Type, Decimal128Type, Decimal256Type, Float32Type, Float64Type, Int8Type,
    Int16Type, Int32Type, Int64Type, IntervalMonthDayNanoType, IntervalUnit, Time64MicrosecondType,
    Time64NanosecondType, TimeUnit, TimestampMicrosecondType, TimestampMillisecondType,
    TimestampNanosecondType, TimestampSecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use serde_json::Value;

/// Converts one cell to JSON; `type_name` is DuckDB's name for the column type.
/// Nested children convert with an empty type name. Types not handled yet give `Null`.
pub fn cell_json<Tz: TimeZone>(array: &dyn Array, row: usize, type_name: &str, zone: &Tz) -> Value {
    if array.is_null(row) {
        return Value::Null;
    }
    match array.data_type() {
        DataType::Boolean => Value::Bool(array.as_boolean().value(row)),
        DataType::Int8 => integer_json(value::<Int8Type>(array, row).into()),
        DataType::Int16 => integer_json(value::<Int16Type>(array, row).into()),
        DataType::Int32 => integer_json(value::<Int32Type>(array, row).into()),
        DataType::Int64 => integer_json(value::<Int64Type>(array, row).into()),
        DataType::UInt8 => integer_json(value::<UInt8Type>(array, row).into()),
        DataType::UInt16 => integer_json(value::<UInt16Type>(array, row).into()),
        DataType::UInt32 => integer_json(value::<UInt32Type>(array, row).into()),
        DataType::UInt64 => integer_json(value::<UInt64Type>(array, row).into()),
        DataType::Float32 => float_json(value::<Float32Type>(array, row).into()),
        DataType::Float64 => float_json(value::<Float64Type>(array, row)),
        DataType::Decimal128(..) => decimal128_json(array.as_primitive(), row, type_name),
        DataType::Decimal256(..) => decimal256_json(array.as_primitive(), row, type_name),
        DataType::Utf8 => string_json(array.as_string::<i32>().value(row)),
        DataType::LargeUtf8 => string_json(array.as_string::<i64>().value(row)),
        DataType::Utf8View => string_json(array.as_string_view().value(row)),
        DataType::Binary => binary_json(array.as_binary::<i32>().value(row), type_name),
        DataType::LargeBinary => binary_json(array.as_binary::<i64>().value(row), type_name),
        DataType::BinaryView => hex_json(array.as_binary_view().value(row)),
        DataType::FixedSizeBinary(_) => hex_json(array.as_fixed_size_binary().value(row)),
        DataType::Date32 => date_json(value::<Date32Type>(array, row)),
        DataType::Time64(TimeUnit::Microsecond) => {
            time_json(split(value::<Time64MicrosecondType>(array, row), 1_000_000))
        }
        DataType::Time64(TimeUnit::Nanosecond) => time_json(split(
            value::<Time64NanosecondType>(array, row),
            1_000_000_000,
        )),
        DataType::Timestamp(unit, time_zone) => {
            let (ticks, per_second) = match unit {
                TimeUnit::Second => (value::<TimestampSecondType>(array, row), 1),
                TimeUnit::Millisecond => (value::<TimestampMillisecondType>(array, row), 1_000),
                TimeUnit::Microsecond => (value::<TimestampMicrosecondType>(array, row), 1_000_000),
                TimeUnit::Nanosecond => {
                    (value::<TimestampNanosecondType>(array, row), 1_000_000_000)
                }
            };
            let zone = time_zone.is_some().then_some(zone);
            timestamp_json(split(ticks, per_second), zone)
        }
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            interval_json(value::<IntervalMonthDayNanoType>(array, row))
        }
        DataType::List(_) => list_json(array.as_list::<i32>().value(row).as_ref(), zone),
        DataType::LargeList(_) => list_json(array.as_list::<i64>().value(row).as_ref(), zone),
        DataType::FixedSizeList(..) => {
            list_json(array.as_fixed_size_list().value(row).as_ref(), zone)
        }
        DataType::Struct(fields) => Value::Object(
            fields
                .iter()
                .zip(array.as_struct().columns())
                .map(|(field, column)| {
                    (
                        field.name().clone(),
                        cell_json(column.as_ref(), row, "", zone),
                    )
                })
                .collect(),
        ),
        DataType::Map(..) => map_json(array.as_map().value(row).columns(), zone),
        DataType::Dictionary(..) => downcast_dictionary_array!(
            array => array
                .key(row)
                .map_or(Value::Null, |key| cell_json(array.values().as_ref(), key, type_name, zone)),
            _ => Value::Null
        ),
        _ => Value::Null,
    }
}

fn list_json<Tz: TimeZone>(items: &dyn Array, zone: &Tz) -> Value {
    Value::Array(
        (0..items.len())
            .map(|index| cell_json(items, index, "", zone))
            .collect(),
    )
}

/// `entries` holds the key and value columns; each key is rendered by `python_key_str`.
fn map_json<Tz: TimeZone>(entries: &[duckdb::arrow::array::ArrayRef], zone: &Tz) -> Value {
    let [keys, values] = entries else {
        return Value::Null;
    };
    Value::Object(
        (0..keys.len())
            .map(|index| {
                (
                    python_key_str(keys.as_ref(), index, zone),
                    cell_json(values.as_ref(), index, "", zone),
                )
            })
            .collect(),
    )
}

/// Renders a map key the way Python `str()` renders the value DuckDB's Python client returns.
fn python_key_str<Tz: TimeZone>(array: &dyn Array, row: usize, zone: &Tz) -> String {
    match array.data_type() {
        DataType::Boolean if array.as_boolean().value(row) => "True".to_owned(),
        DataType::Boolean => "False".to_owned(),
        DataType::Float32 => float_key(value::<Float32Type>(array, row).into()),
        DataType::Float64 => float_key(value::<Float64Type>(array, row)),
        DataType::Decimal128(..) => array.as_primitive::<Decimal128Type>().value_as_string(row),
        DataType::Decimal256(..) => array.as_primitive::<Decimal256Type>().value_as_string(row),
        data_type => match cell_json(array, row, "", zone) {
            Value::String(text) if matches!(data_type, DataType::Timestamp(..)) => {
                text.replacen('T', " ", 1)
            }
            Value::String(text) => text,
            other => other.to_string(),
        },
    }
}

fn float_key(number: f64) -> String {
    if number.is_nan() {
        "nan".to_owned()
    } else {
        python_float_repr(number)
    }
}

#[cfg(test)]
mod tests;
