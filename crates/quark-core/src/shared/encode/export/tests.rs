//! Unit tests for export cell conversion, CSV text and CSV line quoting.

use super::*;
use chrono::{FixedOffset, Utc};
use chrono::{NaiveDate, NaiveTime, TimeDelta};
use duckdb::Connection;

fn cells_in<Tz: TimeZone>(sql: &str, type_name: &str, zone: &Tz) -> Vec<ExportCell> {
    let conn = Connection::open_in_memory().unwrap();
    let mut statement = conn.prepare(sql).unwrap();
    statement
        .query_arrow([])
        .unwrap()
        .flat_map(|batch| {
            let column = batch.column(0).clone();
            (0..column.len())
                .map(|row| export_cell(column.as_ref(), row, type_name, zone))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The cell of a one-value `SELECT`, in UTC.
fn cell(expression: &str, type_name: &str) -> ExportCell {
    cells_in(&format!("SELECT {expression}"), type_name, &Utc).remove(0)
}

fn text(value: &str) -> ExportCell {
    ExportCell::Text(value.to_owned())
}

fn csv(expression: &str, type_name: &str) -> Option<String> {
    csv_text(&cell(expression, type_name))
}

mod decimals;

#[test]
fn scalars_keep_their_types() {
    assert_eq!(cell("NULL::BIGINT", "BIGINT"), ExportCell::Null);
    assert_eq!(cell("true", "BOOLEAN"), ExportCell::Bool(true));
    assert_eq!(cell("42::TINYINT", "TINYINT"), ExportCell::Int(42));
    assert_eq!(cell("1.5::FLOAT", "FLOAT"), ExportCell::Float(1.5));
    assert_eq!(cell("'héllo'", "VARCHAR"), text("héllo"));
}

#[test]
fn big_integers_become_text() {
    let sql = "SELECT * FROM (VALUES (999999999999999::BIGINT), (1000000000000000), \
               (-999999999999999), (-1000000000000000)) t(v)";
    assert_eq!(
        cells_in(sql, "BIGINT", &Utc),
        [
            ExportCell::Int(999_999_999_999_999),
            text("1000000000000000"),
            ExportCell::Int(-999_999_999_999_999),
            text("-1000000000000000"),
        ]
    );
    assert_eq!(
        cell("18446744073709551615::UBIGINT", "UBIGINT"),
        text("18446744073709551615")
    );
    assert_eq!(cell("7::HUGEINT", "HUGEINT"), ExportCell::Int(7));
    assert_eq!(
        cell(
            "170141183460469231731687303715884105727::HUGEINT",
            "HUGEINT"
        ),
        text("170141183460469231731687303715884105727")
    );
    assert_eq!(
        cell(
            "340282366920938463463374607431768211455::UHUGEINT",
            "UHUGEINT"
        ),
        text("340282366920938463463374607431768211455")
    );
}

#[test]
fn small_decimals_stay_decimals() {
    assert_eq!(
        cell("12.50::DECIMAL(10,2)", "DECIMAL(10,2)"),
        ExportCell::Decimal("12.50".to_owned())
    );
    assert_eq!(
        cell("-0.001::DECIMAL(4,3)", "DECIMAL(4,3)"),
        ExportCell::Decimal("-0.001".to_owned())
    );
}

#[test]
fn wide_decimals_become_text() {
    assert_eq!(
        cell("1234567890123456::DECIMAL(18,0)", "DECIMAL(18,0)"),
        text("1234567890123456")
    );
    assert_eq!(
        cell("123456789012345::DECIMAL(18,0)", "DECIMAL(18,0)"),
        ExportCell::Decimal("123456789012345".to_owned())
    );
    assert_eq!(
        cell("1.23456789012345678::DECIMAL(38,17)", "DECIMAL(38,17)"),
        text("1.23456789012345678")
    );
}

#[test]
fn non_finite_floats_become_text() {
    let sql =
        "SELECT * FROM (VALUES ('nan'::DOUBLE), ('inf'::DOUBLE), ('-inf'::DOUBLE), (2.5)) t(v)";
    assert_eq!(
        cells_in(sql, "DOUBLE", &Utc),
        [
            text("NaN"),
            text("Infinity"),
            text("-Infinity"),
            ExportCell::Float(2.5)
        ]
    );
}

#[test]
fn blobs_are_lowercase_hex() {
    assert_eq!(cell("from_hex('00FF')", "BLOB"), text("00ff"));
}

#[test]
fn uuid_enum_bit_and_json_become_text() {
    let uuid = "123e4567-e89b-12d3-a456-426614174000";
    assert_eq!(cell(&format!("'{uuid}'::UUID"), "UUID"), text(uuid));
    assert_eq!(cell("'b'::ENUM('a', 'b')", "ENUM('a', 'b')"), text("b"));
    assert_eq!(cell("'0101'::BIT", "BIT"), text("0101"));
    assert_eq!(cell("'{\"a\":1}'::JSON", "JSON"), text("{\"a\":1}"));
}

#[test]
fn temporal_values_keep_their_types() {
    let date = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();
    assert_eq!(cell("DATE '2024-01-05'", "DATE"), ExportCell::Date(date));
    assert_eq!(
        cell("TIMESTAMP '2024-01-05 10:30:00.000123'", "TIMESTAMP"),
        ExportCell::DateTime(date.and_hms_micro_opt(10, 30, 0, 123).unwrap())
    );
    assert_eq!(
        cell("TIME '10:30:00.5'", "TIME"),
        ExportCell::Time(NaiveTime::from_hms_micro_opt(10, 30, 0, 500_000).unwrap())
    );
}

#[test]
fn zoned_values_become_isoformat_text_in_the_given_zone() {
    let zone = FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
    assert_eq!(
        cells_in(
            "SELECT TIMESTAMPTZ '2024-01-05 10:30:00+00'",
            "TIMESTAMP WITH TIME ZONE",
            &zone
        ),
        [text("2024-01-05T16:00:00+05:30")]
    );
    assert_eq!(
        cells_in(
            "SELECT TIMETZ '12:30:00+05:30'",
            "TIME WITH TIME ZONE",
            &zone
        ),
        [text("12:30:00+05:30")]
    );
}

#[test]
fn intervals_become_durations_with_30_day_months() {
    let expected = TimeDelta::days(32) + TimeDelta::seconds(3);
    assert_eq!(
        cell(
            "INTERVAL 1 MONTH + INTERVAL 2 DAY + INTERVAL 3 SECOND",
            "INTERVAL"
        ),
        ExportCell::Duration(expected)
    );
}

#[test]
fn text_loses_illegal_xml_characters() {
    assert_eq!(
        cell(
            "'a' || chr(1) || 'b' || chr(31) || chr(9) || chr(65535)",
            "VARCHAR"
        ),
        text("ab\t")
    );
}

#[test]
fn nested_values_use_python_json_separators() {
    assert_eq!(cell("[1, NULL, 2]", "INTEGER[]"), text("[1, null, 2]"));
    assert_eq!(
        cell("{'x': 1, 'y': 'é'}", "STRUCT"),
        text(r#"{"x": 1, "y": "é"}"#)
    );
    assert_eq!(cell("[1, 2]::INTEGER[2]", "INTEGER[2]"), text("[1, 2]"));
}

#[test]
fn map_keys_use_python_str() {
    assert_eq!(
        cell("MAP([1, 2], ['a', 'b'])", "MAP"),
        text(r#"{"1": "a", "2": "b"}"#)
    );
    assert_eq!(cell("MAP([true], [1])", "MAP"), text(r#"{"True": 1}"#));
}

#[test]
fn nested_floats_use_python_repr() {
    assert_eq!(
        cell("[1.0, 1e16, 0.1, '-0']::DOUBLE[]", "DOUBLE[]"),
        text("[1.0, 1e+16, 0.1, -0.0]")
    );
}

#[test]
fn nested_non_finite_floats_become_strings() {
    assert_eq!(
        cell("['nan'::DOUBLE, 'inf', '-inf']", "DOUBLE[]"),
        text(r#"["NaN", "Infinity", "-Infinity"]"#)
    );
}

#[test]
fn nested_big_integers_decimals_and_blobs_become_strings() {
    assert_eq!(
        cell("[1000000000000000::BIGINT, 999999999999999]", "BIGINT[]"),
        text(r#"["1000000000000000", 999999999999999]"#)
    );
    assert_eq!(
        cell("[1.50::DECIMAL(5,2)]", "DECIMAL(5,2)[]"),
        text(r#"["1.50"]"#)
    );
    assert_eq!(cell("[from_hex('0aFF')]", "BLOB[]"), text(r#"["0aff"]"#));
}

#[test]
fn nested_temporal_values_use_isoformat() {
    assert_eq!(
        cell("[DATE '2024-01-05']", "DATE[]"),
        text(r#"["2024-01-05"]"#)
    );
    assert_eq!(
        cell("[TIMESTAMP '2024-01-05 10:30:00']", "TIMESTAMP[]"),
        text(r#"["2024-01-05T10:30:00"]"#)
    );
    assert_eq!(
        cell("[TIME '10:30:00.5']", "TIME[]"),
        text(r#"["10:30:00.500000"]"#)
    );
}

#[test]
fn nested_intervals_use_python_timedelta_text() {
    assert_eq!(
        cell("[INTERVAL 1 DAY + INTERVAL 2 HOUR]", "INTERVAL[]"),
        text(r#"["1 day, 2:00:00"]"#)
    );
}

#[test]
fn nested_text_is_json_escaped_without_ascii_escaping() {
    assert_eq!(
        cell("['a\"b', 'é', chr(10), chr(1)]", "VARCHAR[]"),
        text("[\"a\\\"b\", \"é\", \"\\n\", \"\\u0001\"]")
    );
}

#[test]
fn nested_structs_in_lists_keep_field_order() {
    assert_eq!(
        cell("[{'b': 1, 'a': NULL}]", "STRUCT[]"),
        text(r#"[{"b": 1, "a": null}]"#)
    );
}

#[test]
fn csv_text_formats_scalars_like_python() {
    assert_eq!(csv("NULL::INTEGER", "INTEGER"), None);
    assert_eq!(csv("true", "BOOLEAN").as_deref(), Some("True"));
    assert_eq!(csv("false", "BOOLEAN").as_deref(), Some("False"));
    assert_eq!(csv("42", "INTEGER").as_deref(), Some("42"));
    assert_eq!(csv("1e16::DOUBLE", "DOUBLE").as_deref(), Some("1e+16"));
    assert_eq!(csv("2.0::DOUBLE", "DOUBLE").as_deref(), Some("2.0"));
    assert_eq!(
        csv("12.50::DECIMAL(10,2)", "DECIMAL(10,2)").as_deref(),
        Some("12.50")
    );
    assert_eq!(
        csv("DATE '2024-01-05'", "DATE").as_deref(),
        Some("2024-01-05")
    );
    assert_eq!(csv("TIME '10:30:00'", "TIME").as_deref(), Some("10:30:00"));
    assert_eq!(
        csv("TIME '10:30:00.25'", "TIME").as_deref(),
        Some("10:30:00.250000")
    );
}

#[test]
fn naive_datetimes_use_a_space() {
    assert_eq!(
        csv("TIMESTAMP '2024-01-05 10:30:00'", "TIMESTAMP").as_deref(),
        Some("2024-01-05 10:30:00")
    );
    assert_eq!(
        csv("TIMESTAMP '2024-01-05 10:30:00.000123'", "TIMESTAMP").as_deref(),
        Some("2024-01-05 10:30:00.000123")
    );
}

#[test]
fn durations_use_python_timedelta_text() {
    for (expression, expected) in [
        ("INTERVAL 1 DAY + INTERVAL 2 HOUR", "1 day, 2:00:00"),
        ("INTERVAL 2 DAY", "2 days, 0:00:00"),
        ("INTERVAL (-1) SECOND", "-1 day, 23:59:59"),
        ("INTERVAL 90 MINUTE", "1:30:00"),
        ("INTERVAL 1500 MILLISECOND", "0:00:01.500000"),
    ] {
        assert_eq!(
            csv(expression, "INTERVAL").as_deref(),
            Some(expected),
            "{expression}"
        );
    }
}

#[test]
fn formula_prefixes_are_neutralized() {
    for prefix in ["=", "+", "-", "@", "\t", "\r"] {
        let cell = text(&format!("{prefix}SUM(A1)"));
        assert_eq!(csv_text(&cell), Some(format!("'{prefix}SUM(A1)")));
    }
    assert_eq!(csv_text(&text("SUM(A1)")).as_deref(), Some("SUM(A1)"));
    assert_eq!(csv_text(&text("")).as_deref(), Some(""));
}

#[test]
fn numbers_are_never_prefixed() {
    assert_eq!(csv_text(&ExportCell::Int(-5)).as_deref(), Some("-5"));
    assert_eq!(csv_text(&ExportCell::Float(-1.5)).as_deref(), Some("-1.5"));
    assert_eq!(
        csv_text(&ExportCell::Decimal("-1.50".to_owned())).as_deref(),
        Some("-1.50")
    );
}

fn line(fields: &[Option<&str>]) -> String {
    csv_line(
        &fields
            .iter()
            .map(|f| f.map(str::to_owned))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn csv_lines_quote_only_when_needed() {
    assert_eq!(line(&[Some("a"), None, Some("b c")]), "a,,b c\r\n");
    assert_eq!(
        line(&[Some("a,b"), Some("say \"hi\""), Some("x\ny"), Some("x\ry")]),
        "\"a,b\",\"say \"\"hi\"\"\",\"x\ny\",\"x\ry\"\r\n"
    );
}

#[test]
fn a_single_empty_field_is_written_as_quotes() {
    assert_eq!(line(&[None]), "\"\"\r\n");
    assert_eq!(line(&[Some("")]), "\"\"\r\n");
    assert_eq!(line(&[None, None]), ",\r\n");
}
