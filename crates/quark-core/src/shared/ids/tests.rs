//! Unit tests for deterministic ids and Python-compatible JSON.

use super::*;
use serde_json::json;

#[test]
fn dataset_id_matches_python() {
    assert_eq!(dataset_id("main", "data"), "WyJtYWluIiwiZGF0YSJd");
}

#[test]
fn python_json_keeps_ascii_text_and_escapes_the_rest() {
    assert_eq!(
        python_json(&json!(["main", "zoë📊"])),
        r#"["main","zo\u00eb\ud83d\udcca"]"#
    );
    assert_eq!(
        python_json(&json!("a\"b\\c\n\r\t\u{8}\u{c}\u{1}\u{7f}")),
        r#""a\"b\\c\n\r\t\b\f\u0001\u007f""#
    );
}

#[test]
fn dataset_id_encodes_non_ascii_names() {
    assert_eq!(
        dataset_id("main", "zoë📊"),
        "WyJtYWluIiwiem9cdTAwZWJcdWQ4M2RcdWRjY2EiXQ"
    );
}

#[test]
fn view_id_nests_the_dataset_id() {
    let dataset = dataset_id("main", "data");
    assert_eq!(
        view_id("p1", "s1", &dataset),
        "WyJwMSIsInMxIiwiV3lKdFlXbHVJaXdpWkdGMFlTSmQiXQ"
    );
}

#[test]
fn source_schema_prefixes_the_encoded_pair() {
    let dataset = dataset_id("main", "data");
    assert_eq!(
        source_schema("s1", &dataset),
        "source_WyJzMSIsIld5SnRZV2x1SWl3aVpHRjBZU0pkIl0"
    );
}

#[test]
fn database_alias_encodes_the_raw_source_id() {
    assert_eq!(database_alias("0f8e2c"), "database_MGY4ZTJj");
}
