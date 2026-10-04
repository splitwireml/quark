//! Classification of DuckDB column types and JSON conversion of Arrow cells, mirroring the Python backend.

use chrono::TimeZone;
use duckdb::arrow::array::{
    Array, ArrowPrimitiveType, AsArray, PrimitiveArray, downcast_dictionary_array,
};
use duckdb::arrow::datatypes::{
    DataType, Decimal128Type, Decimal256Type, Float32Type, Float64Type, Int8Type, Int16Type,
    Int32Type, Int64Type, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use serde_json::{Number, Value};

/// Largest integer a JSON consumer can hold exactly (2^53 - 1).
const MAX_SAFE_INTEGER: u128 = (1 << 53) - 1;

const NUMERIC: [&str; 14] = [
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "HUGEINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "UHUGEINT",
    "FLOAT",
    "REAL",
    "DOUBLE",
    "DECIMAL",
];
const TEXT: [&str; 3] = ["VARCHAR", "CHAR", "TEXT"];
const DATE: [&str; 3] = ["DATE", "TIME", "TIMESTAMP"];
const NATIVE: [&str; 14] = [
    "BOOLEAN",
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "FLOAT",
    "REAL",
    "DOUBLE",
    "VARCHAR",
    "BLOB",
];

/// True when the uppercased type starts with one of the prefixes.
fn has_prefix(type_: &str, prefixes: &[&str]) -> bool {
    let upper = type_.to_ascii_uppercase();
    prefixes.iter().any(|prefix| upper.starts_with(prefix))
}

pub fn is_numeric(type_: &str) -> bool {
    has_prefix(type_, &NUMERIC)
}

pub fn is_text(type_: &str) -> bool {
    has_prefix(type_, &TEXT)
}

pub fn profile_kind(type_: &str) -> Option<&'static str> {
    let upper = type_.to_ascii_uppercase();
    if is_numeric(type_) {
        Some("numeric")
    } else if is_text(type_) || upper.starts_with("ENUM") || upper == "BOOLEAN" {
        Some("categorical")
    } else if has_prefix(type_, &DATE) {
        Some("date")
    } else {
        None
    }
}

pub fn is_native(type_: &str) -> bool {
    NATIVE.contains(&type_.to_ascii_uppercase().as_str())
}

/// Converts one cell to JSON; `type_name` is DuckDB's name for the column type.
/// Types not handled yet give `Null`.
pub fn cell_json<Tz: TimeZone>(
    array: &dyn Array,
    row: usize,
    type_name: &str,
    _zone: &Tz,
) -> Value {
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
        DataType::Binary => hex_json(array.as_binary::<i32>().value(row)),
        DataType::LargeBinary => hex_json(array.as_binary::<i64>().value(row)),
        DataType::BinaryView => hex_json(array.as_binary_view().value(row)),
        DataType::FixedSizeBinary(_) => hex_json(array.as_fixed_size_binary().value(row)),
        DataType::Dictionary(..) => downcast_dictionary_array!(
            array => array
                .key(row)
                .map_or(Value::Null, |key| cell_json(array.values().as_ref(), key, type_name, _zone)),
            _ => Value::Null
        ),
        _ => Value::Null,
    }
}

fn value<T: ArrowPrimitiveType>(array: &dyn Array, row: usize) -> T::Native {
    array.as_primitive::<T>().value(row)
}

fn integer_json(number: i128) -> Value {
    if number.unsigned_abs() <= MAX_SAFE_INTEGER {
        Value::from(number as i64)
    } else {
        Value::String(number.to_string())
    }
}

fn float_json(number: f64) -> Value {
    Number::from_f64(number).map_or(Value::Null, Value::Number)
}

/// DuckDB exports HUGEINT as Decimal128(38, 0) and UHUGEINT as the same bits reinterpreted.
fn decimal128_json(array: &PrimitiveArray<Decimal128Type>, row: usize, type_name: &str) -> Value {
    let raw = array.value(row);
    match type_name.to_ascii_uppercase().as_str() {
        "HUGEINT" => integer_json(raw),
        "UHUGEINT" => match i128::try_from(raw as u128) {
            Ok(number) => integer_json(number),
            Err(_) => Value::String((raw as u128).to_string()),
        },
        _ => decimal_float(&array.value_as_string(row)),
    }
}

fn decimal256_json(array: &PrimitiveArray<Decimal256Type>, row: usize, type_name: &str) -> Value {
    let raw = array.value(row);
    match type_name.to_ascii_uppercase().as_str() {
        "HUGEINT" | "UHUGEINT" => raw
            .to_i128()
            .map_or_else(|| Value::String(raw.to_string()), integer_json),
        _ => decimal_float(&array.value_as_string(row)),
    }
}

/// `text` is the exact decimal rendering, so parsing it rounds correctly.
fn decimal_float(text: &str) -> Value {
    text.parse().map_or(Value::Null, float_json)
}

fn string_json(text: &str) -> Value {
    Value::String(text.to_owned())
}

fn hex_json(bytes: &[u8]) -> Value {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    Value::String(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use duckdb::Connection;
    use serde_json::json;

    /// Every value of the first column of `sql`, converted with `cell_json`.
    fn cells(sql: &str, type_name: &str) -> Vec<Value> {
        let conn = Connection::open_in_memory().unwrap();
        let mut statement = conn.prepare(sql).unwrap();
        statement
            .query_arrow([])
            .unwrap()
            .flat_map(|batch| {
                let column = batch.column(0).clone();
                (0..column.len())
                    .map(|row| cell_json(column.as_ref(), row, type_name, &Utc))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn big_integers_become_strings() {
        let sql = "SELECT * FROM (VALUES (9007199254740991::BIGINT), (9007199254740992), \
                   (-9007199254740992), (-9007199254740991)) t(v)";
        assert_eq!(
            cells(sql, "BIGINT"),
            [
                json!(9007199254740991_i64),
                json!("9007199254740992"),
                json!("-9007199254740992"),
                json!(-9007199254740991_i64),
            ]
        );
        assert_eq!(
            cells("SELECT 18446744073709551615::UBIGINT", "UBIGINT"),
            [json!("18446744073709551615")]
        );
        assert_eq!(cells("SELECT 42::UTINYINT", "UTINYINT"), [json!(42)]);
        assert_eq!(
            cells(
                "SELECT 170141183460469231731687303715884105727::HUGEINT",
                "HUGEINT"
            ),
            [json!("170141183460469231731687303715884105727")]
        );
        assert_eq!(
            cells(
                "SELECT 340282366920938463463374607431768211455::UHUGEINT",
                "UHUGEINT"
            ),
            [json!("340282366920938463463374607431768211455")]
        );
        assert_eq!(cells("SELECT 7::HUGEINT", "HUGEINT"), [json!(7)]);
    }

    #[test]
    fn floats_drop_non_finite() {
        let sql = "SELECT * FROM (VALUES (1.5::DOUBLE), ('nan'::DOUBLE), ('inf'::DOUBLE), \
                   ('-inf'::DOUBLE)) t(v)";
        assert_eq!(
            cells(sql, "DOUBLE"),
            [json!(1.5), Value::Null, Value::Null, Value::Null]
        );
        assert_eq!(cells("SELECT 0.5::FLOAT", "FLOAT"), [json!(0.5)]);
    }

    #[test]
    fn decimals_become_floats() {
        assert_eq!(
            cells("SELECT 1.25::DECIMAL(18,2)", "DECIMAL(18,2)"),
            [json!(1.25)]
        );
        assert_eq!(
            cells("SELECT 12.345::DECIMAL(38,3)", "DECIMAL(38,3)"),
            [json!(12.345)]
        );
    }

    #[test]
    fn blobs_hex_and_strings() {
        assert_eq!(cells("SELECT from_hex('00ff')", "BLOB"), [json!("00ff")]);
        assert_eq!(cells("SELECT 'héllo'", "VARCHAR"), [json!("héllo")]);
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        assert_eq!(
            cells(&format!("SELECT '{uuid}'::UUID"), "UUID"),
            [json!(uuid)]
        );
        assert_eq!(
            cells("SELECT 'b'::ENUM('a', 'b')", "ENUM('a', 'b')"),
            [json!("b")]
        );
        assert_eq!(
            cells("SELECT '{\"a\":1}'::JSON", "JSON"),
            [json!("{\"a\":1}")]
        );
    }

    #[test]
    fn null_slots_are_null() {
        for (sql, type_name) in [
            ("SELECT NULL::BIGINT", "BIGINT"),
            ("SELECT NULL::DOUBLE", "DOUBLE"),
            ("SELECT NULL::DECIMAL(10,2)", "DECIMAL(10,2)"),
            ("SELECT NULL::VARCHAR", "VARCHAR"),
            ("SELECT NULL::BLOB", "BLOB"),
            ("SELECT NULL::BOOLEAN", "BOOLEAN"),
            ("SELECT NULL::ENUM('a')", "ENUM('a')"),
        ] {
            assert_eq!(cells(sql, type_name), [Value::Null], "{sql}");
        }
    }

    #[test]
    fn booleans_pass_through() {
        assert_eq!(
            cells("SELECT * FROM (VALUES (true), (false)) t(v)", "BOOLEAN"),
            [json!(true), json!(false)]
        );
    }

    #[test]
    fn classification_matches_python() {
        let rows = [
            ("DECIMAL(18,3)", true, Some("numeric"), false),
            ("VARCHAR", false, Some("categorical"), true),
            ("ENUM('a', 'b')", false, Some("categorical"), false),
            ("BOOLEAN", false, Some("categorical"), true),
            ("TIMESTAMP WITH TIME ZONE", false, Some("date"), false),
            ("INTEGER[]", true, Some("numeric"), false),
            ("VARCHAR[]", false, Some("categorical"), false),
            ("INTERVAL", false, None, false),
            ("BLOB", false, None, true),
            ("HUGEINT", true, Some("numeric"), false),
        ];
        for (type_, numeric, kind, native) in rows {
            assert_eq!(is_numeric(type_), numeric, "is_numeric({type_})");
            assert_eq!(profile_kind(type_), kind, "profile_kind({type_})");
            assert_eq!(is_native(type_), native, "is_native({type_})");
        }
    }

    #[test]
    fn wide_decimals_follow_the_same_rules() {
        use duckdb::arrow::array::Decimal256Array;
        use duckdb::arrow::datatypes::i256;

        let values = [i256::from_i128(12345), i256::from_i128(i128::MAX)];
        let array = Decimal256Array::from_iter_values(values)
            .with_precision_and_scale(76, 0)
            .unwrap();

        assert_eq!(cell_json(&array, 0, "HUGEINT", &Utc), json!(12345));
        assert_eq!(
            cell_json(&array, 1, "UHUGEINT", &Utc),
            json!("170141183460469231731687303715884105727")
        );
        assert_eq!(cell_json(&array, 0, "DECIMAL(76,0)", &Utc), json!(12345.0));
    }
}
