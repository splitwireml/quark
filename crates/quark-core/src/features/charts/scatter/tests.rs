//! Tests for the scatter chart, run on real in-memory engines.

use super::super::model::VisualizeRequest;
use super::super::service::visualize;
use crate::error::ApiResult;
use crate::relation::target::Target;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use serde_json::{Value, json};

fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn big_engine() -> Engine {
    engine(
        "CREATE TABLE t AS SELECT i AS x, i * 2 AS y, i AS weight, 'r' || (i % 3) AS region \
         FROM range(200000) AS r(i)",
    )
}

fn scatter(engine: &Engine, encodings: Value, filters: Value) -> ApiResult<Value> {
    let request: VisualizeRequest = serde_json::from_value(json!({
        "spec": {"chart": "scatter", "encodings": encodings},
        "filters": filters,
    }))
    .unwrap();
    let target = Target::Dataset(dataset_id("main", "t"));
    engine
        .read(None, |conn| visualize(engine, conn, &target, &request))
        .unwrap()
}

fn detail(result: ApiResult<Value>) -> String {
    let error = result.unwrap_err();
    assert_eq!(error.status().as_u16(), 422);
    error.detail().to_owned()
}

#[test]
fn sample_is_repeatable() {
    let engine = big_engine();
    let encodings = json!({"x": "x", "y": "y", "color": "region", "size": "weight"});

    let runs: Vec<Value> = (0..5)
        .map(|_| scatter(&engine, encodings.clone(), json!([])).unwrap()["points"].clone())
        .collect();

    assert_eq!(runs[0].as_array().unwrap().len(), 5000);
    assert!(runs.iter().all(|points| points == &runs[0]));
}

#[test]
fn sample_draws_from_rows_that_can_be_plotted() {
    let engine = engine(
        "CREATE TABLE t AS SELECT i AS x, CASE WHEN i % 2 = 0 THEN NULL ELSE i END AS y \
         FROM range(200000) AS r(i)",
    );

    let card = scatter(&engine, json!({"x": "x", "y": "y"}), json!([])).unwrap();

    assert_eq!(card["total_points"], json!(100000));
    assert_eq!(card["points"].as_array().unwrap().len(), 5000);
}

#[test]
fn timetz_shape_labels_read_like_python() {
    let engine = engine(
        "CREATE TABLE t AS SELECT i AS x, i AS y, \
         CASE WHEN i % 3 = 0 THEN TIMETZ '12:00:00+02' WHEN i % 3 = 1 THEN TIMETZ '08:30:00-05:30' \
         ELSE TIMETZ '09:00:00+00' END AS moment FROM range(12) AS r(i)",
    );

    let card = scatter(
        &engine,
        json!({"x": "x", "y": "y", "pattern": "moment"}),
        json!([]),
    )
    .unwrap();

    let labels = card["shape_labels"].as_array().unwrap();
    assert_eq!(
        labels,
        &[
            json!("09:00:00+00:00"),
            json!("12:00:00+02:00"),
            json!("08:30:00-05:30")
        ],
        "equal counts tie-break on the time value, not on its text"
    );
    let mut shapes: Vec<&Value> = card["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| &point["shape"])
        .collect();
    shapes.sort_by_key(|shape| shape.to_string());
    shapes.dedup();
    assert_eq!(shapes.len(), 3);
    assert!(shapes.iter().all(|shape| labels.contains(shape)));
}

/// Spec §6.1: domains come from the whole filtered relation, not the 5000-point sample.
#[test]
fn domains_cover_the_whole_relation() {
    let engine = big_engine();
    let filters = json!([{"column": "x", "operator": "<", "value": 100000}]);

    let card = scatter(
        &engine,
        json!({"x": "x", "y": "y", "size": "weight", "color": "weight"}),
        filters,
    )
    .unwrap();

    assert_eq!(card["points"].as_array().unwrap().len(), 5000);
    assert_eq!(card["total_points"], json!(100000));
    assert_eq!(card["size_domain"], json!([0, 99999]));
    assert_eq!(card["color_kind"], json!("numeric"));
    assert_eq!(card["color_domain"], json!([0, 99999]));
    assert!(card.get("color_labels").is_none());
}

#[test]
fn labels_fold_into_other() {
    let engine = engine(
        "CREATE TABLE t AS SELECT i AS x, i AS y, 'c' || (i % 8) AS region, i % 8 AS tier \
         FROM range(40) AS r(i)",
    );

    let card = scatter(
        &engine,
        json!({"x": "x", "y": "y", "color": "region", "pattern": "tier"}),
        json!([]),
    )
    .unwrap();

    let labels = card["color_labels"].as_array().unwrap();
    assert_eq!(labels.len(), 7);
    assert_eq!(labels[6], json!("Other"));
    assert_eq!(labels[0], json!("c0"));
    assert_eq!(card["shape_labels"][1], json!("1"));
    let folded = card["points"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|point| point["color"] == json!("Other"))
        .count();
    assert_eq!(folded, 10);
    assert!(card["points"][0]["shape"].is_string());
    assert!(card.get("size_domain").is_none());
}

#[test]
fn few_labels_get_no_other_and_bad_coordinates_are_dropped() {
    let engine = engine(
        "CREATE TABLE t AS SELECT * FROM (VALUES (1.0, 2.0, 'a'), (NULL, 3.0, 'a'), \
         ('inf'::DOUBLE, 4.0, 'b'), (5.0, 'nan'::DOUBLE, 'b'), (6.0, 7.0, NULL)) AS v(x, y, region)",
    );

    let card = scatter(
        &engine,
        json!({"x": "x", "y": "y", "color": "region"}),
        json!([]),
    )
    .unwrap();

    assert_eq!(card["total_points"], json!(2));
    assert_eq!(card["color_labels"], json!(["a"]));
    let mut colors: Vec<Value> = card["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["color"].clone())
        .collect();
    colors.sort_by_key(Value::to_string);
    assert_eq!(colors, [json!("a"), Value::Null]);
}

#[test]
fn non_numeric_size_is_422() {
    let engine = big_engine();

    let result = scatter(
        &engine,
        json!({"x": "x", "y": "y", "size": "region"}),
        json!([]),
    );

    assert_eq!(detail(result), "Scatter size requires a numeric column");
}

#[test]
fn missing_encodings_and_columns_are_422_in_order() {
    let engine = big_engine();

    let no_y = scatter(&engine, json!({"x": "x"}), json!([]));
    let unknown_x = scatter(&engine, json!({"x": "nope", "y": "y"}), json!([]));
    let color = scatter(
        &engine,
        json!({"x": "x", "y": "y", "color": "nope"}),
        json!([]),
    );
    let shape = scatter(
        &engine,
        json!({"x": "x", "y": "y", "size": "region", "pattern": "nope"}),
        json!([]),
    );

    assert_eq!(detail(no_y), "Scatter requires x and y encodings");
    assert_eq!(detail(unknown_x), "Scatter requires x and y encodings");
    assert_eq!(detail(color), "Scatter color column was not found");
    assert_eq!(detail(shape), "Scatter shape column was not found");
}
