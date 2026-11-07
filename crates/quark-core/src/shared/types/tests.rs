//! Unit tests for column type classification.

use super::*;

#[test]
fn classification_matches_python() {
    let rows = [
        ("DECIMAL(18,3)", true, Some("numeric"), false),
        ("VARCHAR", false, Some("categorical"), true),
        ("ENUM('a', 'b')", false, Some("categorical"), false),
        ("BOOLEAN", false, Some("categorical"), true),
        ("TIMESTAMP WITH TIME ZONE", false, Some("date"), false),
        ("INTEGER[]", true, Some("numeric"), false),
        ("VARCHAR[]", false, Some("categorical"), false),
        ("INTERVAL", false, None, false),
        ("BLOB", false, None, true),
        ("HUGEINT", true, Some("numeric"), false),
    ];
    for (type_, numeric, kind, native) in rows {
        assert_eq!(is_numeric(type_), numeric, "is_numeric({type_})");
        assert_eq!(profile_kind(type_), kind, "profile_kind({type_})");
        assert_eq!(is_native(type_), native, "is_native({type_})");
    }
}

#[test]
fn is_integer_matches_python() {
    for type_ in [
        "TINYINT",
        "bigint",
        "HUGEINT",
        "UHUGEINT",
        "UBIGINT",
        "INTEGER[]",
    ] {
        assert!(is_integer(type_), "is_integer({type_})");
    }
    for type_ in ["DOUBLE", "DECIMAL(18,3)", "VARCHAR", "FLOAT", "DATE"] {
        assert!(!is_integer(type_), "is_integer({type_})");
    }
}
