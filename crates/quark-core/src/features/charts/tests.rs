//! Tests for chart dispatch and the metric card, run on real in-memory engines.

use super::model::VisualizeRequest;
use super::service::visualize;
use crate::error::ApiResult;
use crate::relation::target::Target;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use serde_json::{Value, json};

pub(super) fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn formula_engine() -> Engine {
    engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES ('east', 10), ('east', 30), ('west', 40)) AS v(region, amount)",
    )
}

pub(super) fn dataset() -> Target {
    Target::Dataset(dataset_id("main", "t"))
}

pub(super) fn run(engine: &Engine, target: &Target, request: Value) -> ApiResult<Value> {
    let request: VisualizeRequest = serde_json::from_value(request).unwrap();
    engine
        .read(None, |conn| visualize(engine, conn, target, &request))
        .unwrap()
}

fn metric(engine: &Engine, spec: Value) -> ApiResult<Value> {
    run(engine, &dataset(), json!({"spec": spec}))
}

pub(super) fn detail(result: ApiResult<Value>) -> String {
    let error = result.unwrap_err();
    assert_eq!(error.status().as_u16(), 422);
    error.detail().to_owned()
}

#[test]
fn metric_counts_rows_by_default() {
    let engine = formula_engine();

    let card = metric(&engine, json!({"chart": "metric"})).unwrap();

    assert_eq!(
        (&card["chart"], &card["value"], &card["rows"]),
        (&json!("metric"), &json!(3), &json!(3))
    );
    assert!(card["elapsed_ms"].is_f64());
    let sum = metric(
        &engine,
        json!({"chart": "metric", "encodings": {"value": "amount"}, "metric": "sum"}),
    )
    .unwrap();
    assert_eq!((&sum["value"], &sum["rows"]), (&json!(80), &json!(3)));
    let distinct = metric(
        &engine,
        json!({"chart": "metric", "encodings": {"value": "region"}, "metric": "distinct"}),
    )
    .unwrap();
    assert_eq!(distinct["value"], json!(2));
}

#[test]
fn metric_rejects_missing_and_non_numeric_columns() {
    let engine = formula_engine();

    let missing = metric(
        &engine,
        json!({"chart": "metric", "encodings": {"value": "nope"}}),
    );
    let text_sum = metric(
        &engine,
        json!({"chart": "metric", "encodings": {"value": "region"}, "metric": "sum"}),
    );

    assert_eq!(detail(missing), "Metric column was not found");
    assert_eq!(detail(text_sum), "This metric requires a numeric column");
}

#[test]
fn metric_formula_binds_the_view_filters() {
    let engine = formula_engine();
    let filters = json!([{"column": "region", "operator": "=", "value": "east"}]);
    let spec = json!({
        "chart": "metric", "expression": "max(\"amount\")",
        "encodings": {"value": "amount"}, "metric": "sum",
    });

    let by_dataset = run(
        &engine,
        &dataset(),
        json!({"spec": spec, "filters": filters}),
    )
    .unwrap();
    let by_sql = run(
        &engine,
        &Target::Sql("SELECT * FROM t".to_owned()),
        json!({"spec": spec, "filters": filters}),
    )
    .unwrap();
    let ratio = metric(
        &engine,
        json!({"chart": "metric", "expression": "sum(\"amount\") / count(*)"}),
    )
    .unwrap();

    assert_eq!(by_dataset["value"], json!(30));
    assert_eq!(by_sql["value"], json!(30));
    assert_eq!(ratio["value"], json!(80.0 / 3.0));
}

#[test]
fn metric_formula_rejects_placeholders_and_multiple_statements() {
    let engine = formula_engine();
    let formula = |expression: &str| {
        metric(
            &engine,
            json!({"chart": "metric", "expression": expression}),
        )
    };

    assert_eq!(
        detail(formula("sum(?)")),
        "A metric formula cannot contain ? placeholders"
    );
    assert!(!detail(formula("1); DROP TABLE t; SELECT (1")).is_empty());
    assert!(detail(formula("sum(\"missing\")")).starts_with("This metric formula did not run: "));
    assert!(detail(formula("\"amount\"")).starts_with("This metric formula did not run: "));
    assert_eq!(formula("count(*)").unwrap()["value"], json!(3));
}

#[test]
fn cached_chart_reports_its_own_elapsed_time() {
    let engine = engine("CREATE TABLE t AS SELECT range AS amount FROM range(8000000)");
    let spec = json!({"chart": "metric", "encodings": {"value": "amount"}, "metric": "median"});

    let first = metric(&engine, spec.clone()).unwrap();
    let again = metric(&engine, spec).unwrap();

    assert_eq!(first["value"], again["value"]);
    let (cold, warm) = (
        first["elapsed_ms"].as_f64().unwrap(),
        again["elapsed_ms"].as_f64().unwrap(),
    );
    assert!(cold > 5.0, "the first call computes: {cold} ms");
    assert!(
        warm < cold / 2.0,
        "the cached call times itself: {warm} ms vs {cold} ms"
    );
}

#[test]
fn metric_formula_with_volatile_functions_is_never_cached() {
    let engine = formula_engine();
    let formulas = [
        "max(random())",
        "sum(amount * random())",
        "max(amount) + (SELECT max(x) FROM query('SELECT ran' || 'dom() AS x'))",
    ];

    for expression in formulas {
        let spec = json!({"chart": "metric", "expression": expression});
        let first = metric(&engine, spec.clone()).unwrap();
        let again = metric(&engine, spec).unwrap();
        assert_ne!(first["value"], again["value"], "{expression} was cached");
    }
}

#[test]
fn metric_formula_over_stable_columns_is_cached() {
    let engine = engine("CREATE TABLE t AS SELECT range AS amount FROM range(8000000)");
    let spec = json!({"chart": "metric", "expression": "median(\"amount\")"});

    let first = metric(&engine, spec.clone()).unwrap();
    let again = metric(&engine, spec).unwrap();

    assert_eq!(first["value"], again["value"]);
    let (cold, warm) = (
        first["elapsed_ms"].as_f64().unwrap(),
        again["elapsed_ms"].as_f64().unwrap(),
    );
    assert!(cold > 5.0, "the first call computes: {cold} ms");
    assert!(
        warm < cold / 2.0,
        "the formula is cached: {warm} ms vs {cold} ms"
    );
}
