//! Unit tests for numeric profiles.

use super::*;
use serde_json::json;

fn profile_of(setup: &str, type_name: &str) -> Value {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(setup).unwrap();
    let meta = ColumnMeta {
        name: "c".to_owned(),
        type_name: type_name.to_owned(),
    };
    let relation = Relation {
        sql: "(SELECT * FROM t)".to_owned(),
        params: Vec::new(),
        columns: vec![meta.clone()],
    };
    profile(&conn, &relation, &meta).unwrap()
}

#[test]
fn float_bins_match_python_edges() {
    let profile = profile_of(
        "CREATE TABLE t AS SELECT i::DOUBLE / 4 AS c FROM range(10) r(i) UNION ALL SELECT NULL",
        "DOUBLE",
    );

    assert_eq!(profile["kind"], "numeric");
    assert_eq!(profile["row_count"], 11);
    assert_eq!(profile["null_count"], 1);
    assert_eq!(profile["min"], json!(0.0));
    assert_eq!(profile["max"], json!(2.25));
    assert_eq!(profile["median"], json!(1.125));
    let width = 2.25 / 4.0;
    let edges: Vec<f64> = (0..4).map(|i| f64::from(i) * width).collect();
    let bins = profile["histogram"].as_array().unwrap();
    assert_eq!(bins.len(), 4);
    for (index, bin) in bins.iter().enumerate() {
        assert_eq!(bin["lower"], json!(edges[index]));
    }
    assert_eq!(bins[3]["upper"], json!(2.25));
    let counts: Vec<u64> = bins.iter().map(|b| b["count"].as_u64().unwrap()).collect();
    assert_eq!(counts, [3, 2, 2, 3]);
}

#[test]
fn single_value_column_has_one_bin() {
    let profile = profile_of("CREATE TABLE t AS SELECT 7 AS c FROM range(3)", "INTEGER");

    assert_eq!(
        profile["histogram"],
        json!([{"lower": 7, "upper": 7, "count": 3}])
    );
    assert_eq!(profile["stddev"], json!(0.0));
}

#[test]
fn huge_integers_use_ntile_bins_with_string_bounds() {
    let profile = profile_of(
        "CREATE TABLE t AS SELECT 9007199254740993::BIGINT + i AS c FROM range(4) r(i)",
        "BIGINT",
    );

    assert_eq!(profile["min"], "9007199254740993");
    let bins = profile["histogram"].as_array().unwrap();
    assert_eq!(bins.len(), 2);
    assert_eq!(
        bins[0],
        json!({"lower": "9007199254740993", "upper": "9007199254740994", "count": 2})
    );
    assert_eq!(bins[1]["upper"], "9007199254740996");
}

#[test]
fn non_finite_values_are_left_out_of_finite_stats() {
    let profile = profile_of(
        "CREATE TABLE t AS SELECT * FROM (VALUES (1.0::DOUBLE), (2.0), (3.0), ('nan'::DOUBLE), \
         ('inf'::DOUBLE), ('-inf'::DOUBLE), (NULL)) v(c)",
        "DOUBLE",
    );

    assert_eq!(profile["row_count"], 7);
    assert_eq!(profile["non_null_count"], 6);
    assert_eq!(profile["null_count"], 1);
    assert_eq!(profile["min"], json!(1.0));
    assert_eq!(profile["max"], json!(3.0));
    assert_eq!(profile["mean"], json!(2.0));
    let counted: u64 = profile["histogram"]
        .as_array()
        .unwrap()
        .iter()
        .map(|bin| bin["count"].as_u64().unwrap())
        .sum();
    assert_eq!(counted, 3);
}

#[test]
fn decimal_bounds_become_floats() {
    let profile = profile_of(
        "CREATE TABLE t AS SELECT (i * 0.5)::DECIMAL(10,2) AS c FROM range(5) r(i)",
        "DECIMAL(10,2)",
    );

    assert_eq!(profile["min"], json!(0.0));
    assert_eq!(profile["max"], json!(2.0));
    let bins = profile["histogram"].as_array().unwrap();
    assert_eq!(bins[0]["lower"], json!(0.0));
    assert_eq!(bins[2]["upper"], json!(2.0));
    let counted: u64 = bins.iter().map(|bin| bin["count"].as_u64().unwrap()).sum();
    assert_eq!(counted, 5);
}

#[test]
fn empty_column_has_no_histogram() {
    let profile = profile_of("CREATE TABLE t (c DOUBLE)", "DOUBLE");

    assert_eq!(profile["row_count"], 0);
    assert_eq!(profile["null_fraction"], json!(0.0));
    assert_eq!(profile["histogram"], json!([]));
}
