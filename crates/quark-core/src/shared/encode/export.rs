//! Converts Arrow cells to the typed values and CSV text the exports write, like the Python backend.

use super::cell::cell_json;
use super::scalar::{split, value};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeDelta, TimeZone, Utc};
use duckdb::arrow::array::{Array, AsArray};
use duckdb::arrow::datatypes::{
    DataType, Date32Type, Decimal128Type, Decimal256Type, Float32Type, Float64Type, Int8Type,
    Int16Type, Int32Type, Int64Type, IntervalMonthDayNanoType, IntervalUnit, Time64MicrosecondType,
    Time64NanosecondType, TimeUnit, TimestampMicrosecondType, TimestampMillisecondType,
    TimestampNanosecondType, TimestampSecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use nested::write_nested;
use serde_json::Value;

/// Integers beyond this magnitude (10^15 - 1) are exported as text.
const MAX_EXPORT_INTEGER: u128 = 999_999_999_999_999;
/// Decimals with more digits than this are exported as text.
const MAX_DECIMAL_DIGITS: usize = 15;

/// One exported value; the variants mirror what the Python export hands its writers.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportCell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Python's `str(Decimal)`.
    Decimal(String),
    Text(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Time(NaiveTime),
    Duration(TimeDelta),
}

/// Converts one cell like Python's `export_value`; `type_name` is DuckDB's name for the column type.
pub fn export_cell<Tz: TimeZone>(
    array: &dyn Array,
    row: usize,
    type_name: &str,
    zone: &Tz,
) -> ExportCell {
    match plain_cell(array, row, type_name, zone) {
        ExportCell::Text(text) => ExportCell::Text(strip_illegal(text)),
        other => other,
    }
}

fn is_illegal_xml(c: char) -> bool {
    matches!(c, '\0'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')
}

fn strip_illegal(text: String) -> String {
    if text.contains(is_illegal_xml) {
        text.chars().filter(|c| !is_illegal_xml(*c)).collect()
    } else {
        text
    }
}

/// The conversion before illegal characters are stripped, which `export_nested` must not do.
fn plain_cell<Tz: TimeZone>(
    array: &dyn Array,
    row: usize,
    type_name: &str,
    zone: &Tz,
) -> ExportCell {
    if array.is_null(row) {
        return ExportCell::Null;
    }
    match array.data_type() {
        DataType::Boolean => ExportCell::Bool(array.as_boolean().value(row)),
        DataType::Int8 => int_cell(value::<Int8Type>(array, row).into()),
        DataType::Int16 => int_cell(value::<Int16Type>(array, row).into()),
        DataType::Int32 => int_cell(value::<Int32Type>(array, row).into()),
        DataType::Int64 => int_cell(value::<Int64Type>(array, row).into()),
        DataType::UInt8 => int_cell(value::<UInt8Type>(array, row).into()),
        DataType::UInt16 => int_cell(value::<UInt16Type>(array, row).into()),
        DataType::UInt32 => int_cell(value::<UInt32Type>(array, row).into()),
        DataType::UInt64 => int_cell(value::<UInt64Type>(array, row).into()),
        DataType::Float32 => float_cell(value::<Float32Type>(array, row).into()),
        DataType::Float64 => float_cell(value::<Float64Type>(array, row)),
        DataType::Decimal128(..) => decimal_cell(array, row, type_name),
        DataType::Decimal256(..) => {
            decimal_text(array.as_primitive::<Decimal256Type>().value_as_string(row))
        }
        DataType::Date32 => {
            DateTime::from_timestamp(i64::from(value::<Date32Type>(array, row)) * 86_400, 0)
                .map_or(ExportCell::Null, |date| ExportCell::Date(date.date_naive()))
        }
        DataType::Time64(unit) => {
            let (seconds, micros) = match unit {
                TimeUnit::Nanosecond => {
                    split(value::<Time64NanosecondType>(array, row), 1_000_000_000)
                }
                _ => split(value::<Time64MicrosecondType>(array, row), 1_000_000),
            };
            u32::try_from(seconds)
                .ok()
                .and_then(|seconds| {
                    NaiveTime::from_num_seconds_from_midnight_opt(seconds, micros * 1000)
                })
                .map_or(ExportCell::Null, |time| time_cell(time, type_name, zone))
        }
        DataType::Timestamp(unit, None) => {
            let (seconds, micros) = match unit {
                TimeUnit::Second => split(value::<TimestampSecondType>(array, row), 1),
                TimeUnit::Millisecond => {
                    split(value::<TimestampMillisecondType>(array, row), 1_000)
                }
                TimeUnit::Microsecond => {
                    split(value::<TimestampMicrosecondType>(array, row), 1_000_000)
                }
                TimeUnit::Nanosecond => {
                    split(value::<TimestampNanosecondType>(array, row), 1_000_000_000)
                }
            };
            DateTime::from_timestamp(seconds, micros * 1000).map_or(ExportCell::Null, |instant| {
                ExportCell::DateTime(instant.naive_utc())
            })
        }
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            let (months, days, nanos) =
                IntervalMonthDayNanoType::to_parts(value::<IntervalMonthDayNanoType>(array, row));
            let micros = (i128::from(months) * 30 + i128::from(days)) * 86_400_000_000
                + i128::from(nanos / 1000);
            i64::try_from(micros).map_or(ExportCell::Null, |micros| {
                ExportCell::Duration(TimeDelta::microseconds(micros))
            })
        }
        DataType::List(_)
        | DataType::LargeList(_)
        | DataType::FixedSizeList(..)
        | DataType::Struct(_)
        | DataType::Map(..) => {
            let mut json = String::new();
            write_nested(&mut json, array, row, zone);
            ExportCell::Text(json)
        }
        _ => match cell_json(array, row, type_name, zone) {
            Value::String(text) => ExportCell::Text(text),
            _ => ExportCell::Null,
        },
    }
}

/// Arrow drops a TIMETZ's offset, so its time of day is shown with `zone`'s current offset, as text.
fn time_cell<Tz: TimeZone>(time: NaiveTime, type_name: &str, zone: &Tz) -> ExportCell {
    if !type_name.to_ascii_uppercase().starts_with("TIME WITH") {
        return ExportCell::Time(time);
    }
    let offset = zone.offset_from_utc_datetime(&Utc::now().naive_utc()).fix();
    let text = csv_text(&ExportCell::Time(time)).unwrap_or_default();
    ExportCell::Text(format!("{text}{offset}"))
}

fn int_cell(number: i128) -> ExportCell {
    match i64::try_from(number) {
        Ok(small) if number.unsigned_abs() <= MAX_EXPORT_INTEGER => ExportCell::Int(small),
        _ => ExportCell::Text(number.to_string()),
    }
}

fn float_cell(number: f64) -> ExportCell {
    match number {
        number if number.is_nan() => ExportCell::Text("NaN".to_owned()),
        f64::INFINITY => ExportCell::Text("Infinity".to_owned()),
        f64::NEG_INFINITY => ExportCell::Text("-Infinity".to_owned()),
        number => ExportCell::Float(number),
    }
}

/// DuckDB exports HUGEINT as Decimal128(38, 0) and UHUGEINT as the same bits reinterpreted.
fn decimal_cell(array: &dyn Array, row: usize, type_name: &str) -> ExportCell {
    let column = array.as_primitive::<Decimal128Type>();
    let raw = column.value(row);
    match type_name.to_ascii_uppercase().as_str() {
        "HUGEINT" => int_cell(raw),
        "UHUGEINT" => i128::try_from(raw as u128)
            .map_or_else(|_| ExportCell::Text((raw as u128).to_string()), int_cell),
        _ => decimal_text(column.value_as_string(row)),
    }
}

/// Python counts a `Decimal`'s digits without its leading zeros.
fn decimal_text(text: String) -> ExportCell {
    let digits = text
        .bytes()
        .filter(u8::is_ascii_digit)
        .skip_while(|digit| *digit == b'0')
        .count();
    if digits > MAX_DECIMAL_DIGITS {
        ExportCell::Text(text)
    } else {
        ExportCell::Decimal(text)
    }
}

mod csv;
mod nested;

pub use csv::{csv_line, csv_text};

#[cfg(test)]
mod tests;
