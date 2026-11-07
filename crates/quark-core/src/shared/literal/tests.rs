//! Unit tests for SQL literals and Python float formatting.

use super::*;
use serde_json::json;

#[test]
fn float_repr_matches_python() {
    let cases = [
        (0.1, "0.1"),
        (1.0, "1.0"),
        (-0.0, "-0.0"),
        (1e16, "1e+16"),
        (1e15, "1000000000000000.0"),
        (1.5e-05, "1.5e-05"),
        (0.0001, "0.0001"),
        (123456789.123, "123456789.123"),
        (1e22, "1e+22"),
        (f64::MAX, "1.7976931348623157e+308"),
        (5e-324, "5e-324"),
        (2.5, "2.5"),
        (100.0, "100.0"),
    ];
    for (input, expected) in cases {
        assert_eq!(python_float_repr(input), expected, "input {input:?}");
    }
}

#[test]
fn literal_renders_each_json_kind() {
    assert_eq!(literal(&Value::Null), "NULL");
    assert_eq!(literal(&json!(true)), "TRUE");
    assert_eq!(literal(&json!(false)), "FALSE");
    assert_eq!(literal(&json!(42)), "42");
    assert_eq!(literal(&json!(-7)), "-7");
    assert_eq!(
        literal(&json!(18446744073709551615u64)),
        "18446744073709551615"
    );
    assert_eq!(literal(&json!(2.5)), "2.5");
    assert_eq!(literal(&json!("O'Brien")), "'O''Brien'");
    assert_eq!(literal(&json!([1, "a"])), "'[1,\"a\"]'");
    assert_eq!(literal(&json!({"k": "it's"})), "'{\"k\":\"it''s\"}'");
}
