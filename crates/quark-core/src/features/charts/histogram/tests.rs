//! Tests for histograms: shared bin edges, series for every bin layout and the checks.

use serde_json::{Value, json};

use crate::features::charts::model::VisualizeRequest;
use crate::features::charts::service::visualize;
use crate::relation::filtered::filtered;
use crate::relation::request::QueryRequest;
use crate::relation::target::Target;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use duckdb::Connection;

fn engine(setup: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let kind = EngineKind::Node {
        node_id: "s1".to_owned(),
    };
    Engine::new(kind, conn).unwrap()
}

fn histogram(engine: &Engine, encodings: Value) -> Result<Value, String> {
    let request = json!({"spec": {"chart": "histogram", "encodings": encodings}});
    let request: VisualizeRequest = serde_json::from_value(request).unwrap();
    let target = Target::Dataset(dataset_id("main", "t"));
    engine
        .read(None, |conn| visualize(engine, conn, &target, &request))
        .unwrap()
        .map_err(|error| error.detail().to_owned())
}

fn counts(bins: &Value) -> Vec<i64> {
    bins.as_array()
        .unwrap()
        .iter()
        .map(|bin| bin["count"].as_i64().unwrap())
        .collect()
}

fn edges(bins: &Value) -> Vec<(Value, Value)> {
    bins.as_array()
        .unwrap()
        .iter()
        .map(|bin| (bin["lower"].clone(), bin["upper"].clone()))
        .collect()
}

fn series_of<'a>(payload: &'a Value, label: &str) -> &'a Value {
    payload["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|series| series["label"] == label)
        .unwrap_or_else(|| panic!("no series {label}"))
}

/// Every series shares the main bins' edges, and the series counts add up to each bin's total.
fn assert_series_cover_the_bins(payload: &Value) {
    let totals = counts(&payload["bins"]);
    let mut sums = vec![0; totals.len()];
    for series in payload["series"].as_array().unwrap() {
        assert_eq!(edges(&series["bins"]), edges(&payload["bins"]));
        for (sum, count) in sums.iter_mut().zip(counts(&series["bins"])) {
            *sum += count;
        }
    }
    assert_eq!(sums, totals);
}

#[test]
fn uniform_bins_use_the_width_formula() {
    let engine = engine(
        "CREATE TABLE t AS SELECT i AS amount, 'g' || (i % 8) AS region \
         FROM range(1, 201) AS r(i)",
    );

    let payload = histogram(&engine, json!({"value": "amount", "group": "region"})).unwrap();

    assert_eq!(payload["chart"], "histogram");
    assert!(payload["elapsed_ms"].is_f64());
    let labels: Vec<&str> = payload["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|series| series["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels.len(), 7);
    assert_eq!(labels[6], "Other");
    assert_series_cover_the_bins(&payload);
    assert_eq!(counts(&payload["bins"]).iter().sum::<i64>(), 200);
}

#[test]
fn date_bins_group_by_edges() {
    let engine = engine(
        "CREATE TABLE t AS SELECT DATE '2026-01-01' + CAST(i AS INTEGER) AS day, \
         CASE WHEN i % 2 = 0 THEN 'east' ELSE 'west' END AS region FROM range(28) AS r(i)",
    );

    let payload = histogram(&engine, json!({"value": "day", "group": "region"})).unwrap();

    assert_series_cover_the_bins(&payload);
    assert_eq!(
        series_of(&payload, "east")["bins"]
            .as_array()
            .unwrap()
            .len(),
        payload["bins"].as_array().unwrap().len()
    );
    let total: i64 = ["east", "west"]
        .iter()
        .map(|label| {
            counts(&series_of(&payload, label)["bins"])
                .iter()
                .sum::<i64>()
        })
        .sum();
    assert_eq!(total, 28);
}

#[test]
fn timestamp_bins_group_by_edges() {
    let engine = engine(
        "CREATE TABLE t AS SELECT TIMESTAMP '2026-01-01 00:00:00' + to_hours(i) AS at, \
         CASE WHEN i % 3 = 0 THEN 'a' ELSE 'b' END AS region FROM range(40) AS r(i)",
    );

    let payload = histogram(&engine, json!({"value": "at", "group": "region"})).unwrap();

    assert_series_cover_the_bins(&payload);
}

#[test]
fn single_bin_groups_by_series() {
    let engine = engine(
        "CREATE TABLE t AS SELECT 5 AS amount, CASE WHEN i < 3 THEN 'a' ELSE 'b' END AS region \
         FROM range(5) AS r(i)",
    );

    let payload = histogram(&engine, json!({"value": "amount", "group": "region"})).unwrap();

    assert_eq!(counts(&payload["bins"]), vec![5]);
    assert_eq!(counts(&series_of(&payload, "a")["bins"]), vec![3]);
    assert_eq!(counts(&series_of(&payload, "b")["bins"]), vec![2]);
}

#[test]
fn no_group_omits_series() {
    let engine = engine("CREATE TABLE t AS SELECT i AS amount FROM range(1, 5) AS r(i)");

    let payload = histogram(&engine, json!({"value": "amount"})).unwrap();

    assert!(payload.get("series").is_none());
    assert_eq!(counts(&payload["bins"]).iter().sum::<i64>(), 4);
}

#[test]
fn checks_run_in_pythons_order() {
    let engine =
        engine("CREATE TABLE t AS SELECT i AS amount, 'x' AS region FROM range(1, 5) AS r(i)");

    assert_eq!(
        histogram(&engine, json!({})).unwrap_err(),
        "Histogram requires a value encoding"
    );
    assert_eq!(
        histogram(&engine, json!({"value": "nope"})).unwrap_err(),
        "Histogram requires a value encoding"
    );
    assert_eq!(
        histogram(&engine, json!({"value": "region"})).unwrap_err(),
        "Histogram requires a numeric or date column"
    );
    assert_eq!(
        histogram(&engine, json!({"value": "amount", "group": "nope"})).unwrap_err(),
        "Histogram group column was not found"
    );
}

#[test]
fn boolean_groups_are_labelled_the_way_python_prints_them() {
    let engine =
        engine("CREATE TABLE t AS SELECT i AS amount, i % 3 = 0 AS flag FROM range(1, 13) AS r(i)");

    let payload = histogram(&engine, json!({"value": "amount", "group": "flag"})).unwrap();

    let labels: Vec<&str> = payload["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|series| series["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, vec!["False", "True"]);
    assert_eq!(
        counts(&series_of(&payload, "True")["bins"])
            .iter()
            .sum::<i64>(),
        4
    );
    assert_series_cover_the_bins(&payload);
}

#[test]
fn a_histogram_reuses_the_profile_the_stats_panel_cached() {
    let engine = engine("CREATE TABLE t AS SELECT i AS amount FROM range(1, 5) AS r(i)");
    let target = Target::Dataset(dataset_id("main", "t"));
    let seeded = json!({"histogram": [{"lower": 0.0, "upper": 1.0, "count": 99}]});
    engine
        .read(None, |conn| {
            let relation = filtered(conn, &target, &QueryRequest::default()).unwrap();
            engine.cached_response(conn, "stats", &relation, &json!("amount"), || {
                Ok(seeded.clone())
            })
        })
        .unwrap()
        .unwrap();

    let payload = histogram(&engine, json!({"value": "amount"})).unwrap();

    assert_eq!(payload["bins"], seeded["histogram"]);
}
