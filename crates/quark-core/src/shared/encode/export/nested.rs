//! Writes nested list, struct and map cells as the JSON text Python's `json.dumps` produces.

use super::csv::{csv_text, iso_text};
use super::{ExportCell, plain_cell};
use crate::shared::encode::cell::python_str;
use crate::shared::literal::python_float_repr;
use chrono::TimeZone;
use duckdb::arrow::array::{Array, AsArray};
use duckdb::arrow::datatypes::DataType;
use std::fmt::Write;

/// Writes `json.dumps(export_nested(v), ensure_ascii=False)` for a list, struct, map or scalar.
pub(super) fn write_nested<Tz: TimeZone>(
    out: &mut String,
    array: &dyn Array,
    row: usize,
    zone: &Tz,
) {
    if array.is_null(row) {
        out.push_str("null");
        return;
    }
    match array.data_type() {
        DataType::List(_) => write_items(out, array.as_list::<i32>().value(row).as_ref(), zone),
        DataType::LargeList(_) => {
            write_items(out, array.as_list::<i64>().value(row).as_ref(), zone)
        }
        DataType::FixedSizeList(..) => {
            write_items(out, array.as_fixed_size_list().value(row).as_ref(), zone)
        }
        DataType::Struct(fields) => {
            let names = fields.iter().map(|field| field.name().clone());
            let members = names.zip(array.as_struct().columns());
            write_object(
                out,
                members.map(|(name, column)| (name, column.as_ref(), row)),
                zone,
            );
        }
        DataType::Map(..) => {
            if let [keys, values] = array.as_map().value(row).columns() {
                let members = (0..keys.len()).map(|index| {
                    (
                        python_str(keys.as_ref(), index, zone),
                        values.as_ref(),
                        index,
                    )
                });
                write_object(out, members, zone);
            }
        }
        _ => write_scalar(out, &plain_cell(array, row, "", zone)),
    }
}

fn write_items<Tz: TimeZone>(out: &mut String, items: &dyn Array, zone: &Tz) {
    out.push('[');
    for index in 0..items.len() {
        if index > 0 {
            out.push_str(", ");
        }
        write_nested(out, items, index, zone);
    }
    out.push(']');
}

/// `members` are the key, the value column and the row to read in it.
fn write_object<'a, Tz: TimeZone>(
    out: &mut String,
    members: impl Iterator<Item = (String, &'a dyn Array, usize)>,
    zone: &Tz,
) {
    out.push('{');
    for (position, (key, column, row)) in members.enumerate() {
        if position > 0 {
            out.push_str(", ");
        }
        write_json_string(out, &key);
        out.push_str(": ");
        write_nested(out, column, row, zone);
    }
    out.push('}');
}

fn write_scalar(out: &mut String, cell: &ExportCell) {
    match cell {
        ExportCell::Null => out.push_str("null"),
        ExportCell::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        ExportCell::Int(number) => {
            let _ = write!(out, "{number}");
        }
        ExportCell::Float(number) => out.push_str(&python_float_repr(*number)),
        ExportCell::Decimal(text) | ExportCell::Text(text) => write_json_string(out, text),
        ExportCell::Date(_)
        | ExportCell::DateTime(_)
        | ExportCell::Time(_)
        | ExportCell::Duration(_) => {
            let text = match cell {
                ExportCell::DateTime(moment) => iso_text(moment, 'T'),
                other => csv_text(other).unwrap_or_default(),
            };
            write_json_string(out, &text);
        }
    }
}

/// Quotes `text` like Python's `json.dumps(..., ensure_ascii=False)`.
fn write_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
