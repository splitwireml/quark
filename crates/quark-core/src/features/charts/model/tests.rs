//! Unit tests for the chart request types.

use super::*;
use serde_json::json;

fn spec(value: serde_json::Value) -> Result<ChartSpec, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn unknown_chart_kind_is_rejected() {
    assert!(spec(json!({"chart": "donut"})).is_err());
    assert!(spec(json!({"chart": "bar", "metric": "mode"})).is_err());
    assert!(spec(json!({"chart": "bar", "layout": "overlaid"})).is_err());
    assert!(spec(json!({"chart": "bar", "extra": 1})).is_err());
    assert!(spec(json!({})).is_err());
}

#[test]
fn unknown_encoding_field_is_rejected() {
    assert!(spec(json!({"chart": "bar", "encodings": {"z": "a"}})).is_err());
}

#[test]
fn long_expression_is_422() {
    let long = |key: &str, count: usize| {
        let mut value = json!({"chart": "metric"});
        value[key] = json!("é".repeat(count));
        spec(value).unwrap()
    };
    assert!(long("expression", 2000).validate().is_ok());
    assert!(long("label", 200).validate().is_ok());
    for (key, count) in [("expression", 2001), ("label", 201)] {
        let error = long(key, count).validate().unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }

    let request: VisualizeRequest =
        serde_json::from_value(json!({"spec": {"chart": "bar"}, "page": 0})).unwrap();
    assert_eq!(
        request.validate().unwrap_err().detail(),
        "Invalid page or page size"
    );
}

#[test]
fn defaults_match_python() {
    let parsed = spec(json!({"chart": "pie"})).unwrap();
    assert_eq!(parsed.chart, ChartKind::Pie);
    assert_eq!(parsed.encodings, ChartEncodings::default());
    assert!(!parsed.density);
    assert!(parsed.metric.is_none() && parsed.layout.is_none() && parsed.grain.is_none());
    assert!(parsed.expression.is_none() && parsed.label.is_none());
    assert!(parsed.align.is_none() && parsed.font.is_none() && parsed.size.is_none());

    let request: VisualizeRequest =
        serde_json::from_value(json!({"spec": {"chart": "bar"}})).unwrap();
    assert_eq!(request.query(), QueryRequest::default());
    let sql: SqlVisualizeRequest =
        serde_json::from_value(json!({"sql": "SELECT 1", "spec": {"chart": "bar"}})).unwrap();
    assert_eq!(sql.query(), QueryRequest::default());
    assert_eq!(sql.spec, request.spec);
    assert!(
        serde_json::from_value::<SqlVisualizeRequest>(json!({"spec": {"chart": "bar"}})).is_err()
    );
    assert!(
        serde_json::from_value::<VisualizeRequest>(json!({"spec": {"chart": "bar"}, "x": 1}))
            .is_err()
    );
}

#[test]
fn metric_sql_templates() {
    let column = "\"a b\"";
    let expected = [
        (Metric::Count, "count(\"a b\")"),
        (Metric::Distinct, "count(DISTINCT \"a b\")"),
        (Metric::Min, "min(\"a b\")"),
        (Metric::Max, "max(\"a b\")"),
        (Metric::Sum, "sum(\"a b\")"),
        (Metric::Avg, "avg(\"a b\")"),
        (Metric::Median, "median(\"a b\")"),
        (Metric::Stddev, "stddev_samp(\"a b\")"),
    ];
    for (metric, sql) in expected {
        assert_eq!(metric_sql(metric, column), sql);
    }
}
