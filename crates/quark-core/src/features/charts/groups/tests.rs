//! Tests for the series a bar chart's group encoding adds, run on real in-memory engines.

use super::super::tests::{dataset, engine, run};
use crate::runtime::engine::Engine;
use serde_json::{Value, json};

fn grouped(engine: &Engine, category: &str, group: &str) -> Value {
    let spec = json!({
        "chart": "bar", "metric": "sum",
        "encodings": {"category": category, "value": "amount", "group": group},
    });
    run(engine, &dataset(), json!({"spec": spec})).unwrap()
}

fn values(card: &Value, label: &str) -> Value {
    let row = card["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label"] == json!(label))
        .unwrap();
    row["values"].clone()
}

#[test]
fn groups_align_values_to_series() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES
            ('east', 'q1', 10), ('east', 'q2', 20), ('west', 'q1', 30), ('west', 'q2', 40),
            ('west', 'q2', 5), ('north', 'q1', 7), ('south', NULL, 99))
            AS v(region, quarter, amount)",
    );

    let card = grouped(&engine, "region", "quarter");

    assert_eq!(card["series"], json!(["q1", "q2"]));
    assert_eq!(values(&card, "east"), json!([10, 20]));
    assert_eq!(values(&card, "west"), json!([30, 45]));
    assert_eq!(values(&card, "north"), json!([7, 0]));
    assert_eq!(values(&card, "south"), json!([0, 0]));
}

#[test]
fn series_beyond_six_fold_into_other() {
    let engine = engine(
        "CREATE TABLE t AS SELECT 'east' AS region, 't' || i AS tag, i AS amount FROM range(10) a(i)
         UNION ALL SELECT 'east', 't0', 100",
    );

    let card = grouped(&engine, "region", "tag");

    assert_eq!(
        card["series"],
        json!(["t0", "t1", "t2", "t3", "t4", "t5", "Other"])
    );
    assert_eq!(
        values(&card, "east"),
        json!([100, 1, 2, 3, 4, 5, 6 + 7 + 8 + 9])
    );
}

#[test]
fn series_labels_use_python_str() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES
            ('a', 1, TRUE, TIMESTAMP '2024-01-02 03:04:05', 1.5), ('a', 1, FALSE, TIMESTAMP '2024-01-02 03:04:05', 2.5),
            ('a', 2, FALSE, TIMESTAMP '2024-02-03 00:00:00', 2.5))
            AS v(region, amount, flag, stamp, ratio)",
    );

    let flags = grouped(&engine, "region", "flag");
    let stamps = grouped(&engine, "region", "stamp");
    let ratios = grouped(&engine, "region", "ratio");
    let numbers = grouped(&engine, "region", "amount");

    assert_eq!(flags["series"], json!(["False", "True"]));
    assert_eq!(values(&flags, "a"), json!([3, 1]));
    assert_eq!(
        stamps["series"],
        json!(["2024-01-02 03:04:05", "2024-02-03 00:00:00"])
    );
    assert_eq!(ratios["series"], json!(["2.5", "1.5"]));
    assert_eq!(numbers["series"], json!(["1", "2"]));
    assert_eq!(values(&numbers, "a"), json!([2, 2]));
}

#[test]
fn no_series_leaves_the_rows_plain() {
    let engine =
        engine("CREATE TABLE t AS SELECT 'a' AS region, NULL::VARCHAR AS tag, 1 AS amount");

    let card = grouped(&engine, "region", "tag");

    assert_eq!(card["series"], Value::Null);
    assert!(card["rows"][0].get("values").is_none());
}

fn rows_values(card: &Value) -> Value {
    Value::Array(
        card["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["values"].clone())
            .collect(),
    )
}

#[test]
fn timetz_groups_keep_their_offset() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES ('a', TIMETZ '03:04:05+02', 10),
            ('a', TIMETZ '04:04:05+02', 20)) AS v(region, moment, amount)",
    );

    let card = grouped(&engine, "region", "moment");

    assert_eq!(card["series"], json!(["03:04:05+02:00", "04:04:05+02:00"]));
    assert_eq!(values(&card, "a"), json!([10, 20]));
}

#[test]
fn interval_categories_match_their_series() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES (INTERVAL 1 DAY, 'x', 10),
            (INTERVAL 2 DAY, 'y', 20)) AS v(span, tag, amount)",
    );

    let card = grouped(&engine, "span", "tag");

    assert_eq!(card["series"], json!(["x", "y"]));
    assert_eq!(rows_values(&card), json!([[0, 20], [10, 0]]));
}

#[test]
fn nan_bars_keep_zeros_and_nan_series_match() {
    let nan_bars = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES (CAST('nan' AS DOUBLE), 'x', 10),
            (1.5, 'y', 20)) AS v(score, tag, amount)",
    );
    let nan_series = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES ('a', CAST('nan' AS DOUBLE), 10),
            ('a', 1.5, 20), ('a', 1.5, 1)) AS v(region, score, amount)",
    );

    let bars = grouped(&nan_bars, "score", "tag");
    let groups = grouped(&nan_series, "region", "score");

    assert_eq!(bars["series"], json!(["x", "y"]));
    assert_eq!(rows_values(&bars), json!([[0, 20], [0, 0]]));
    assert_eq!(groups["series"], json!(["1.5", "nan"]));
    assert_eq!(values(&groups, "a"), json!([21, 10]));
}

#[test]
fn nan_bar_rows_outside_the_top_series_still_make_other() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (SELECT 1.5::DOUBLE AS score, 't' || (i % 6) AS tag, 10 AS amount
            FROM range(12) r(i)
            UNION ALL SELECT CAST('nan' AS DOUBLE), 't6', 5) ",
    );

    let card = grouped(&engine, "score", "tag");

    assert_eq!(
        card["series"],
        json!(["t0", "t1", "t2", "t3", "t4", "t5", "Other"])
    );
    assert_eq!(
        rows_values(&card),
        json!([[20, 20, 20, 20, 20, 20, 0], [0, 0, 0, 0, 0, 0, 0]])
    );
}
