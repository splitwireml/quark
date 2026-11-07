//! Tests for the bar and pie charts, run on real in-memory engines.

use super::super::tests::{dataset, detail, engine, run};
use crate::runtime::engine::Engine;
use serde_json::{Value, json};

fn chart(engine: &Engine, spec: Value) -> crate::error::ApiResult<Value> {
    run(engine, &dataset(), json!({"spec": spec}))
}

fn labels(card: &Value) -> Vec<&str> {
    card["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["label"].as_str().unwrap())
        .collect()
}

/// Category `c00` has one row, `c01` two, up to `c34` with 35, plus rows with no category.
fn tiered() -> Engine {
    engine(
        "CREATE TABLE t AS
         SELECT CASE WHEN a.i < 35 THEN 'c' || lpad(a.i::VARCHAR, 2, '0') END AS cat, a.i AS amount
         FROM range(36) a(i) JOIN range(40) b(j) ON b.j <= a.i",
    )
}

#[test]
fn bar_counts_top_values_and_other_bucket() {
    let engine = tiered();

    let card = chart(
        &engine,
        json!({"chart": "bar", "encodings": {"category": "cat"}}),
    )
    .unwrap();

    assert_eq!(card["chart"], json!("bar"));
    assert_eq!(card["series"], Value::Null);
    assert!(card["elapsed_ms"].is_f64());
    let shown = labels(&card);
    assert_eq!((shown.len(), shown[0], shown[29]), (30, "c34", "c05"));
    assert_eq!(
        card["rows"][0],
        json!({"label": "c34", "value": 35, "n": 35})
    );
    assert_eq!(card["other_count"], json!(1 + 2 + 3 + 4 + 5));
}

#[test]
fn bar_measure_filters_non_finite_values() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES
            ('a', 1.0), ('a', 3.0), ('a', 'NaN'::DOUBLE), ('a', 'inf'::DOUBLE), ('a', NULL),
            ('b', 10.0), ('c', NULL)) AS v(cat, amount)",
    );

    let average = chart(
        &engine,
        json!({"chart": "bar", "encodings": {"category": "cat", "value": "amount"}}),
    )
    .unwrap();
    let counted = chart(
        &engine,
        json!({"chart": "bar", "encodings": {"category": "cat", "value": "amount"}, "metric": "count"}),
    )
    .unwrap();

    assert_eq!(
        average["rows"],
        json!([{"label": "b", "value": 10.0, "n": 1}, {"label": "a", "value": 2.0, "n": 2}])
    );
    assert_eq!(average["other_count"], json!(0));
    assert_eq!(
        counted["rows"][1],
        json!({"label": "b", "value": 1, "n": 1})
    );
    assert_eq!(counted["rows"][0]["value"], json!(4));
}

#[test]
fn pie_uses_eight_slices() {
    let engine = tiered();

    let card = chart(
        &engine,
        json!({"chart": "pie", "encodings": {"category": "cat", "group": "cat"}}),
    )
    .unwrap();

    assert_eq!(card["chart"], json!("pie"));
    assert_eq!(labels(&card).len(), 8);
    assert_eq!(card["rows"][0].as_object().unwrap().len(), 3);
    assert_eq!(card["other_count"], json!((1..=27).sum::<i64>()));
}

#[test]
fn pie_rejects_negative_values() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES ('a', -4), ('a', -1), ('b', 3)) AS v(cat, amount)",
    );
    let spec = |chart: &str| json!({"chart": chart, "encodings": {"category": "cat", "value": "amount"}, "metric": "sum"});

    let pie = chart(&engine, spec("pie"));
    let bar = chart(&engine, spec("bar")).unwrap();

    assert_eq!(
        detail(pie),
        "A pie chart cannot show negative values; use a bar chart instead"
    );
    assert_eq!(bar["rows"][1]["value"], json!(-5));
}

#[test]
fn one_query_reports_the_non_null_total() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES ('a'), ('a'), ('b'), (NULL), (NULL)) AS v(cat)",
    );
    let spec = json!({"chart": "bar", "encodings": {"category": "cat"}});

    let card = chart(&engine, spec).unwrap();
    let none = run(
        &engine,
        &dataset(),
        json!({"spec": {"chart": "bar", "encodings": {"category": "cat"}},
               "filters": [{"column": "cat", "operator": "=", "value": "zzz"}]}),
    )
    .unwrap();

    assert_eq!(card["other_count"], json!(0));
    assert_eq!(none["rows"], json!([]));
    assert_eq!(none["other_count"], json!(0));
}

#[test]
fn bar_rejects_bad_encodings_with_the_chart_label() {
    let engine = tiered();
    let bad = |chart: &str, encodings: Value, metric: &str| {
        detail(self::chart(
            &engine,
            json!({"chart": chart, "encodings": encodings, "metric": metric}),
        ))
    };

    assert_eq!(
        bad("pie", json!({}), "count"),
        "Pie requires a category encoding"
    );
    assert_eq!(
        bad("bar", json!({"category": "nope"}), "count"),
        "Bar requires a category encoding"
    );
    assert_eq!(
        bad("bar", json!({"category": "cat", "value": "nope"}), "count"),
        "Bar measure column was not found"
    );
    assert_eq!(
        bad("pie", json!({"category": "cat", "value": "cat"}), "sum"),
        "Pie measure requires a numeric column"
    );
}

#[test]
fn bar_rejects_an_unknown_group_column_but_a_pie_ignores_it() {
    let engine = tiered();
    let spec =
        |chart: &str| json!({"chart": chart, "encodings": {"category": "cat", "group": "missing"}});

    let bar = chart(&engine, spec("bar"));
    let pie = chart(&engine, spec("pie"));

    assert_eq!(detail(bar), "Bar group column was not found");
    assert!(pie.is_ok());
}

#[test]
fn bar_treats_an_empty_value_or_group_encoding_as_absent() {
    let engine = tiered();

    let empty_value = chart(
        &engine,
        json!({"chart": "bar", "encodings": {"category": "cat", "value": ""}}),
    )
    .unwrap();
    let empty_group = chart(
        &engine,
        json!({"chart": "bar", "encodings": {"category": "cat", "group": ""}}),
    )
    .unwrap();

    assert_eq!(empty_value["rows"][0]["label"], json!("c34"));
    assert_eq!(empty_value["rows"][0]["value"], json!(35));
    assert_eq!(empty_group["rows"][0]["value"], json!(35));
    assert_eq!(empty_group["series"], Value::Null);
}
