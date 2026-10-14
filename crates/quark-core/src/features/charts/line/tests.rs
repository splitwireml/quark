//! Tests for the line chart, run on real in-memory engines.

use crate::error::ApiResult;
use crate::features::charts::model::VisualizeRequest;
use crate::features::charts::service::visualize;
use crate::relation::target::Target;
use crate::runtime::engine::{Engine, EngineKind};
use crate::shared::ids::dataset_id;
use duckdb::Connection;
use serde_json::{Value, json};

fn line(setup: &str, spec: Value) -> ApiResult<Value> {
    line_filtered(setup, spec, json!([]))
}

fn line_filtered(setup: &str, spec: Value, filters: Value) -> ApiResult<Value> {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let engine = Engine::new(
        EngineKind::Node {
            node_id: "s1".to_owned(),
        },
        conn,
    )
    .unwrap();
    let request: VisualizeRequest =
        serde_json::from_value(json!({"spec": spec, "filters": filters})).unwrap();
    let target = Target::Dataset(dataset_id("main", "t"));
    engine
        .read(None, |conn| {
            conn.execute_batch("SET TimeZone = 'America/New_York'")
                .unwrap();
            visualize(&engine, conn, &target, &request)
        })
        .unwrap()
}

fn spec(extra: Value) -> Value {
    let mut spec =
        json!({"chart": "line", "encodings": {"x": "ts", "y": "amount"}, "metric": "sum"});
    spec.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    spec
}

fn ys(chart: &Value, index: usize) -> Vec<Value> {
    chart["series"][index]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["y"].clone())
        .collect()
}

fn dates(spans: &str) -> String {
    format!("CREATE TABLE t AS SELECT * FROM (VALUES {spans}) AS v(ts, amount)")
}

#[test]
fn automatic_grain_picks_the_first_fit() {
    let grain = |values: &str| line(&dates(values), spec(json!({}))).unwrap()["grain"].clone();

    assert_eq!(
        grain("(TIMESTAMP '2026-01-01 00:00', 1), (TIMESTAMP '2026-01-01 02:00', 1)"),
        "hour"
    );
    assert_eq!(
        grain("(DATE '2026-01-01', 1), (DATE '2026-01-29', 1)"),
        "day"
    );
    assert_eq!(
        grain("(DATE '2020-01-01', 1), (DATE '2025-06-01', 1)"),
        "week"
    );
    assert_eq!(
        grain("(DATE '1900-01-01', 1), (DATE '2100-01-01', 1)"),
        "year"
    );
}

#[test]
fn explicit_grain_is_honoured() {
    let values = "(DATE '2026-01-01', 1), (DATE '2026-01-15', 2), (DATE '2026-02-03', 4)";

    let chart = line(&dates(values), spec(json!({"grain": "month"}))).unwrap();

    assert_eq!(chart["grain"], "month");
    assert_eq!(ys(&chart, 0), [json!(3), json!(4)]);
    let capped = line(
        "CREATE TABLE t AS SELECT TIMESTAMP '2026-01-01' + to_hours(i) AS ts, i AS amount FROM range(500) r(i)",
        spec(json!({"grain": "hour"})),
    )
    .unwrap();
    assert_eq!(ys(&capped, 0).len(), 400);
    assert_eq!(ys(&capped, 0)[399], 499);
}

#[test]
fn series_are_ranked_and_other_comes_last() {
    let mut rows = Vec::new();
    for (group, count) in [
        ("a", 1),
        ("b", 2),
        ("c", 3),
        ("d", 4),
        ("e", 5),
        ("f", 6),
        ("g", 7),
        ("h", 8),
    ] {
        for day in 1..=count {
            rows.push(format!("(DATE '2026-01-{day:02}', '{group}', {day})"));
        }
    }
    rows.push("(DATE '2026-01-01', NULL, 100)".to_owned());
    let setup = format!(
        "CREATE TABLE t AS SELECT * FROM (VALUES {}) AS v(ts, region, amount)",
        rows.join(", ")
    );

    let chart = line(
        &setup,
        spec(json!({"encodings": {"x": "ts", "y": "amount", "group": "region"}})),
    )
    .unwrap();

    let labels: Vec<&str> = chart["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["h", "g", "f", "e", "d", "c", "Other"]);
    assert_eq!(ys(&chart, 0).len(), 8);
    assert_eq!(ys(&chart, 6), [json!(2), json!(2)]);
}

#[test]
fn boolean_groups_are_labelled_the_way_python_prints_them() {
    let setup = "CREATE TABLE t AS SELECT * FROM (VALUES (DATE '2026-01-01', true, 1), \
         (DATE '2026-01-01', true, 2), (DATE '2026-01-02', false, 3)) AS v(ts, flag, amount)";

    let chart = line(
        setup,
        spec(json!({"encodings": {"x": "ts", "y": "amount", "group": "flag"}})),
    )
    .unwrap();

    assert_eq!(chart["series"][0]["label"], "True");
    assert_eq!(chart["series"][1]["label"], "False");
}

#[test]
fn groups_are_labelled_the_way_python_str_prints_them() {
    let label = |group: &str| {
        let setup =
            format!("CREATE TABLE t AS SELECT DATE '2026-01-01' AS ts, {group} AS g, 1 AS amount");
        let chart = line(
            &setup,
            spec(json!({"encodings": {"x": "ts", "y": "amount", "group": "g"}})),
        )
        .unwrap();
        chart["series"][0]["label"].clone()
    };

    assert_eq!(
        label("TIMESTAMP '2024-01-01 00:00:00.5'"),
        "2024-01-01 00:00:00.500000"
    );
    assert_eq!(label("INTERVAL 1 DAY"), "1 day, 0:00:00");
    assert_eq!(label("TIMETZ '12:00:00+02'"), "12:00:00+02:00");
}

#[test]
fn counts_rows_without_a_y_encoding() {
    let setup = "CREATE TABLE t AS SELECT * FROM (VALUES (DATE '2026-01-01'), (DATE '2026-01-01'), (DATE '2026-01-02')) AS v(ts)";

    let chart = line(setup, json!({"chart": "line", "encodings": {"x": "ts"}})).unwrap();

    assert_eq!(chart["series"][0]["label"], "");
    assert_eq!(ys(&chart, 0), [json!(2), json!(1)]);
    let empty = line(
        "CREATE TABLE t AS SELECT DATE '2026-01-01' AS ts WHERE false",
        json!({"chart": "line", "encodings": {"x": "ts"}}),
    )
    .unwrap();
    assert_eq!(empty["series"], json!([{"label": "", "points": []}]));
}

#[test]
fn line_buckets_follow_the_session_zone_across_dst() {
    let setup = dates(
        "(TIMESTAMPTZ '2024-03-10 04:30:00+00', 1), (TIMESTAMPTZ '2024-03-10 06:30:00+00', 1), \
         (TIMESTAMPTZ '2024-03-11 03:30:00+00', 1), (TIMESTAMPTZ '2024-03-11 04:30:00+00', 1)",
    );

    let chart = line(&setup, spec(json!({"grain": "day"}))).unwrap();

    assert_eq!(ys(&chart, 0), [json!(1), json!(2), json!(1)]);
}

#[test]
fn line_rejects_bad_encodings() {
    let setup = "CREATE TABLE t AS SELECT * FROM (VALUES (DATE '2026-01-01', 'x', 1)) AS v(ts, name, amount)";
    let detail = |encodings: Value, extra: Value| {
        let mut spec = spec(extra);
        spec["encodings"] = encodings;
        let error = line(setup, spec).unwrap_err();
        assert_eq!(error.status().as_u16(), 422);
        error.detail().to_owned()
    };

    assert_eq!(detail(json!({}), json!({})), "Line requires an x encoding");
    assert_eq!(
        detail(json!({"x": "nope"}), json!({})),
        "Line requires an x encoding"
    );
    assert_eq!(
        detail(json!({"x": "amount"}), json!({})),
        "Line requires a date or timestamp x column; amount is not one"
    );
    assert_eq!(
        detail(json!({"x": "ts", "y": "nope"}), json!({})),
        "Line measure column was not found"
    );
    assert_eq!(
        detail(json!({"x": "ts", "y": "name"}), json!({})),
        "Line measure requires a numeric column"
    );
    assert_eq!(
        detail(json!({"x": "ts", "group": "nope"}), json!({})),
        "Line group column was not found"
    );
}

#[test]
fn view_filters_bind_before_the_series_query_runs() {
    let setup = "CREATE TABLE t AS SELECT * FROM (VALUES (DATE '2026-01-01', 'a', 1), \
        (DATE '2026-01-01', 'b', 10), (DATE '2026-01-02', 'a', 2), (DATE '2026-01-02', 'b', 20)) \
        AS v(ts, g, amount)";
    let filters = json!([{"column": "g", "operator": "=", "value": "a"}]);
    let encodings = json!({"x": "ts", "y": "amount", "group": "g"});

    let chart = line_filtered(setup, spec(json!({"encodings": encodings})), filters).unwrap();

    let series = chart["series"].as_array().unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0]["label"], "a");
    assert_eq!(ys(&chart, 0), vec![json!(1), json!(2)]);
    assert_eq!(series[0]["points"][0]["x"], "2026-01-01T00:00:00");
}
