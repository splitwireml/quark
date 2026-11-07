//! Tests for the box plot, run on real in-memory engines.

use crate::features::charts::model::VisualizeRequest;
use crate::features::charts::service::visualize;
use crate::relation::target::Target;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use serde_json::{Value, json};

fn run(setup: &str, encodings: Value) -> Value {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    let engine = Engine::new(kind, conn).unwrap();
    let target = Target::Dataset(dataset_id("main", "t"));
    let request: VisualizeRequest =
        serde_json::from_value(json!({"spec": {"chart": "box", "encodings": encodings}})).unwrap();
    engine
        .read(None, |conn| visualize(&engine, conn, &target, &request))
        .unwrap()
        .unwrap()
}

#[test]
fn single_box_has_tukey_whiskers_and_outliers() {
    let box_plot = run(
        "CREATE TABLE t AS SELECT range + 1 AS amount FROM range(10) UNION ALL SELECT 100",
        json!({"value": "amount"}),
    );

    assert_eq!(box_plot["chart"], "box");
    assert!(box_plot["elapsed_ms"].is_f64());
    let groups = box_plot["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0],
        json!({
            "label": "all", "p25": 3.5, "median": 6.0, "p75": 8.5,
            "whisker_low": 1, "whisker_high": 10, "outliers": [100], "count": 11,
        })
    );
}

#[test]
fn groups_keep_the_twelve_largest() {
    let box_plot = run(
        "CREATE TABLE t AS SELECT 'g' || lpad(g::VARCHAR, 2, '0') AS label, v::INTEGER AS amount \
         FROM range(14) a(g), range(20) b(v) WHERE v <= g + 5 \
         UNION ALL SELECT NULL, 1 UNION ALL SELECT 'g99', NULL",
        json!({"value": "amount", "group": "label"}),
    );

    let groups = box_plot["groups"].as_array().unwrap();
    let labels: Vec<&str> = groups
        .iter()
        .map(|g| g["label"].as_str().unwrap())
        .collect();
    let expected: Vec<String> = (2..14).rev().map(|g| format!("g{g:02}")).collect();
    assert_eq!(labels, expected);
    assert_eq!(groups[0]["count"], 19);
}

#[test]
fn outliers_are_capped_at_200_per_group() {
    let box_plot = run(
        "CREATE TABLE t AS SELECT 1 AS amount FROM range(2000) \
         UNION ALL SELECT 1000 + range FROM range(250)",
        json!({"value": "amount"}),
    );

    let group = &box_plot["groups"][0];
    let outliers = group["outliers"].as_array().unwrap();
    assert_eq!(group["count"], 2250);
    assert_eq!(outliers.len(), 200);
    assert_eq!((&outliers[0], &outliers[199]), (&json!(1000), &json!(1199)));
}

#[test]
fn box_rejects_missing_non_numeric_and_unknown_group_columns() {
    let setup = "CREATE TABLE t AS SELECT 'a' AS region, 1 AS amount";
    let expect = |encodings: Value| {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(setup).unwrap();
        let kind = EngineKind::Node {
            node_id: "s1".to_owned(),
        };
        let engine = Engine::new(kind, conn).unwrap();
        let target = Target::Dataset(dataset_id("main", "t"));
        let request: VisualizeRequest =
            serde_json::from_value(json!({"spec": {"chart": "box", "encodings": encodings}}))
                .unwrap();
        let error = engine
            .read(None, |conn| visualize(&engine, conn, &target, &request))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.status().as_u16(), 422);
        error.detail().to_owned()
    };

    assert_eq!(expect(json!({})), "Box requires a value encoding");
    assert_eq!(
        expect(json!({"value": "nope"})),
        "Box requires a value encoding"
    );
    assert_eq!(
        expect(json!({"value": "region"})),
        "Box requires a numeric column"
    );
    assert_eq!(
        expect(json!({"value": "amount", "group": "nope"})),
        "Box group column was not found"
    );
}
