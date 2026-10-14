//! Unit tests for JSON cell conversion of DuckDB values.

use super::*;
use chrono::Utc;
use duckdb::Connection;
use serde_json::json;

/// Every value of the first column of `sql`, converted with `cell_json` in UTC.
fn cells(sql: &str, type_name: &str) -> Vec<Value> {
    cells_in(sql, type_name, &Utc)
}

fn cells_in<Tz: TimeZone>(sql: &str, type_name: &str, zone: &Tz) -> Vec<Value> {
    let conn = Connection::open_in_memory().unwrap();
    let mut statement = conn.prepare(sql).unwrap();
    statement
        .query_arrow([])
        .unwrap()
        .flat_map(|batch| {
            let column = batch.column(0).clone();
            (0..column.len())
                .map(|row| cell_json(column.as_ref(), row, type_name, zone))
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
fn bits_render_like_python() {
    assert_eq!(cells("SELECT '0101'::BIT", "BIT"), [json!("0101")]);
    assert_eq!(
        cells("SELECT '10101010101'::BIT", "BIT"),
        [json!("10101010101")]
    );
    assert_eq!(cells("SELECT NULL::BIT", "BIT"), [Value::Null]);
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
fn dates_and_times_match_python_isoformat() {
    for (sql, type_name, expected) in [
        ("DATE '2024-01-05'", "DATE", "2024-01-05"),
        ("TIME '10:30:00.5'", "TIME", "10:30:00.500000"),
        ("TIME '10:30:00'", "TIME", "10:30:00"),
        (
            "TIMESTAMP '2024-01-05 10:30:00'",
            "TIMESTAMP",
            "2024-01-05T10:30:00",
        ),
        (
            "TIMESTAMP '2024-01-05 10:30:00.000123'",
            "TIMESTAMP",
            "2024-01-05T10:30:00.000123",
        ),
        (
            "TIMESTAMP_NS '2024-01-05 10:30:00.123456789'",
            "TIMESTAMP_NS",
            "2024-01-05T10:30:00.123456",
        ),
        (
            "TIMESTAMP_MS '1969-12-31 23:59:59.999'",
            "TIMESTAMP_MS",
            "1969-12-31T23:59:59.999000",
        ),
        (
            "TIMESTAMP_NS '1969-12-31 23:59:59.999999999'",
            "TIMESTAMP_NS",
            "1969-12-31T23:59:59.999999",
        ),
    ] {
        assert_eq!(
            cells(&format!("SELECT {sql}"), type_name),
            [json!(expected)],
            "{sql}"
        );
    }
}

#[test]
fn timestamptz_uses_given_zone() {
    let zone = chrono::FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
    assert_eq!(
        cells_in(
            "SELECT TIMESTAMPTZ '2024-01-05 10:30:00+00'",
            "TIMESTAMP WITH TIME ZONE",
            &zone
        ),
        [json!("2024-01-05T16:00:00+05:30")]
    );
}

#[test]
fn timestamptz_crosses_dst() {
    let zone = chrono_tz::America::New_York;
    for (instant, expected) in [
        ("2024-03-10 06:59:59+00", "2024-03-10T01:59:59-05:00"),
        ("2024-03-10 07:00:00+00", "2024-03-10T03:00:00-04:00"),
    ] {
        assert_eq!(
            cells_in(
                &format!("SELECT TIMESTAMPTZ '{instant}'"),
                "TIMESTAMP WITH TIME ZONE",
                &zone
            ),
            [json!(expected)],
            "{instant}"
        );
    }
}

#[test]
fn interval_is_total_seconds() {
    assert_eq!(
        cells(
            "SELECT INTERVAL 1 MONTH + INTERVAL 2 DAY + INTERVAL 3 SECOND",
            "INTERVAL"
        ),
        [json!(2764803.0)]
    );
    assert_eq!(
        cells("SELECT INTERVAL '2 days'", "INTERVAL"),
        [json!(172800.0)]
    );
}

#[test]
fn interval_keys_print_like_a_python_timedelta() {
    let keys = |sql: &str| -> Vec<String> {
        let conn = Connection::open_in_memory().unwrap();
        let mut statement = conn.prepare(sql).unwrap();
        let batch = statement.query_arrow([]).unwrap().next().unwrap();
        (0..batch.num_rows())
            .map(|row| python_str(batch.column(0).as_ref(), row, &chrono::Utc))
            .collect()
    };

    assert_eq!(
        keys(
            "SELECT * FROM (VALUES (INTERVAL 1 DAY), (INTERVAL '2 days 3 hours 4 seconds'), \
              (INTERVAL '-1 second'), (INTERVAL '1.5 seconds'), (INTERVAL 1 MONTH)) AS v(i)"
        ),
        [
            "1 day, 0:00:00",
            "2 days, 3:00:04",
            "-1 day, 23:59:59",
            "0:00:01.500000",
            "30 days, 0:00:00"
        ]
    );
}

#[test]
fn lists_structs_maps() {
    for (sql, expected) in [
        ("[1, NULL]", "[1,null]"),
        ("{'x': 1}", r#"{"x":1}"#),
        ("MAP([1,2],['a','b'])", r#"{"1":"a","2":"b"}"#),
        ("[DATE '2024-01-05']", r#"["2024-01-05"]"#),
        ("[1, 2]::INTEGER[2]", "[1,2]"),
        (
            "{'b': 1, 'a': {'d': [2.5], 'c': NULL}}",
            r#"{"b":1,"a":{"d":[2.5],"c":null}}"#,
        ),
        (
            "[{'id': 1, 'tag': 'a'}, {'id': 2, 'tag': NULL}]",
            r#"[{"id":1,"tag":"a"},{"id":2,"tag":null}]"#,
        ),
        ("[9007199254740993]", r#"["9007199254740993"]"#),
        ("MAP(['k'],[[1]])", r#"{"k":[1]}"#),
        ("MAP([true],[1])", r#"{"True":1}"#),
        ("MAP([false],[1])", r#"{"False":1}"#),
        (
            "MAP([TIMESTAMP '2024-01-01 10:00:00'],[1])",
            r#"{"2024-01-01 10:00:00":1}"#,
        ),
        (
            "MAP([TIMESTAMP '2024-01-01 10:00:00.5'],[1])",
            r#"{"2024-01-01 10:00:00.500000":1}"#,
        ),
        ("MAP([1.50::DECIMAL(3,2)],[1])", r#"{"1.50":1}"#),
        ("MAP([7::INTEGER],[1])", r#"{"7":1}"#),
        ("MAP(['a'],[1])", r#"{"a":1}"#),
        ("MAP([1e20::DOUBLE],[1])", r#"{"1e+20":1}"#),
        ("MAP([2.5::DOUBLE],[1])", r#"{"2.5":1}"#),
        ("MAP(['nan'::DOUBLE],[1])", r#"{"nan":1}"#),
        ("MAP([DATE '2024-01-02'],[1])", r#"{"2024-01-02":1}"#),
        ("MAP([TIME '10:00:00'],[1])", r#"{"10:00:00":1}"#),
    ] {
        let cells = cells(&format!("SELECT {sql}"), "");
        assert_eq!(cells[0].to_string(), expected, "{sql}");
    }
    assert_eq!(
        cells(
            "SELECT * FROM (VALUES ([1]), (NULL), ([])) t(v)",
            "INTEGER[]"
        ),
        [json!([1]), Value::Null, json!([])]
    );
}

#[test]
fn map_keys_with_a_time_zone_keep_the_offset() {
    let zone = chrono::FixedOffset::east_opt(4 * 3600).unwrap();
    let cells = cells_in(
        "SELECT MAP([TIMESTAMPTZ '2024-01-01 10:00:00+00'],[1])",
        "",
        &zone,
    );
    assert_eq!(cells[0].to_string(), r#"{"2024-01-01 14:00:00+04:00":1}"#);
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

#[test]
fn timetz_text_renders_like_python() {
    for (literal, expected) in [
        ("12:30:00+02", "12:30:00+02:00"),
        ("12:30:00+05:30", "12:30:00+05:30"),
        ("12:30:00-02", "12:30:00-02:00"),
        ("01:02:03.5+00", "01:02:03.500000+00:00"),
        ("01:02:03.000123-07:30", "01:02:03.000123-07:30"),
    ] {
        let sql = format!("SELECT CAST(TIMETZ '{literal}' AS VARCHAR)");
        assert_eq!(
            cells(&sql, "TIME WITH TIME ZONE"),
            [json!(expected)],
            "{literal}"
        );
    }
    assert_eq!(
        cells("SELECT 'not a time'", "TIME WITH TIME ZONE"),
        [json!("not a time")]
    );
    assert_eq!(
        cells("SELECT '12:30:00+02'", "VARCHAR"),
        [json!("12:30:00+02")]
    );
}
