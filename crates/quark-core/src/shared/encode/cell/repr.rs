//! Python `repr()` of the values inside a list, and `str()` of a blob, for text keys and labels.

use chrono::TimeZone;
use duckdb::arrow::array::{Array, ArrayRef, AsArray};
use duckdb::arrow::datatypes::DataType;

use super::python_str;

/// Python `str(list)`: `[` then each item's `repr()`, joined by `, `, then `]`.
pub(super) fn list_str<Tz: TimeZone>(items: &ArrayRef, zone: &Tz) -> String {
    let shown: Vec<String> = (0..items.len())
        .map(|index| item_repr(items.as_ref(), index, zone))
        .collect();
    format!("[{}]", shown.join(", "))
}

/// Python `repr()` for the item types a list holds; any other type reads as its `str()`.
fn item_repr<Tz: TimeZone>(array: &dyn Array, row: usize, zone: &Tz) -> String {
    if array.is_null(row) {
        return "None".to_owned();
    }
    match array.data_type() {
        DataType::Utf8 => text_repr(array.as_string::<i32>().value(row)),
        DataType::LargeUtf8 => text_repr(array.as_string::<i64>().value(row)),
        DataType::List(_) => list_str(&array.as_list::<i32>().value(row), zone),
        DataType::LargeList(_) => list_str(&array.as_list::<i64>().value(row), zone),
        DataType::FixedSizeList(..) => list_str(&array.as_fixed_size_list().value(row), zone),
        _ => python_str(array, row, zone),
    }
}

/// Python `repr(str)`: single quotes unless the text holds one and no double quote.
fn text_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut shown = String::with_capacity(text.len() + 2);
    shown.push(quote);
    for c in text.chars() {
        match c {
            '\\' => shown.push_str("\\\\"),
            '\n' => shown.push_str("\\n"),
            '\r' => shown.push_str("\\r"),
            '\t' => shown.push_str("\\t"),
            c if c == quote => {
                shown.push('\\');
                shown.push(c);
            }
            c if c.is_ascii_control() => shown.push_str(&format!("\\x{:02x}", u32::from(c))),
            c => shown.push(c),
        }
    }
    shown.push(quote);
    shown
}

/// Python `str(bytes)`: a `b'...'` literal with non-printable bytes as `\xNN`.
pub(super) fn bytes_str(bytes: &[u8]) -> String {
    let quote = if bytes.contains(&b'\'') && !bytes.contains(&b'"') {
        '"'
    } else {
        '\''
    };
    let mut shown = String::with_capacity(bytes.len() + 3);
    shown.push('b');
    shown.push(quote);
    for &byte in bytes {
        match byte {
            b'\\' => shown.push_str("\\\\"),
            b'\n' => shown.push_str("\\n"),
            b'\r' => shown.push_str("\\r"),
            b'\t' => shown.push_str("\\t"),
            byte if char::from(byte) == quote => {
                shown.push('\\');
                shown.push(quote);
            }
            0x20..=0x7e => shown.push(char::from(byte)),
            byte => shown.push_str(&format!("\\x{byte:02x}")),
        }
    }
    shown.push(quote);
    shown
}
