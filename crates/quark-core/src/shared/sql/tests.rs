//! Unit tests for SQL quoting and scan expressions.

use super::*;

#[test]
fn quote_ident_doubles_inner_quotes() {
    assert_eq!(quote_ident("plain"), r#""plain""#);
    assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
}

#[test]
fn sql_string_doubles_single_quotes() {
    assert_eq!(sql_string("plain"), "'plain'");
    assert_eq!(sql_string("it's"), "'it''s'");
}

#[test]
fn scan_expression_per_extension() {
    assert_eq!(
        scan_expression("/d/a.csv").as_deref(),
        Some("read_csv_auto('/d/a.csv', delim=',')")
    );
    assert_eq!(
        scan_expression("/d/a.CSV").as_deref(),
        Some("read_csv_auto('/d/a.CSV', delim=',')")
    );
    assert_eq!(
        scan_expression("/d/a.tsv").as_deref(),
        Some(r"read_csv_auto('/d/a.tsv', delim='\t')")
    );
    assert_eq!(
        scan_expression("/d/a.parquet").as_deref(),
        Some("read_parquet('/d/a.parquet')")
    );
    for ext in ["json", "ndjson", "jsonl"] {
        assert_eq!(
            scan_expression(&format!("/d/a.{ext}")),
            Some(format!("read_json_auto('/d/a.{ext}')"))
        );
    }
    assert_eq!(scan_expression("/d/a.duckdb"), None);
    assert_eq!(scan_expression("/d/a.xlsx"), None);
    assert_eq!(scan_expression("/d/noext"), None);
}

#[test]
fn scan_expression_escapes_quotes_in_paths() {
    assert_eq!(
        scan_expression("/tmp/it's/x.csv").as_deref(),
        Some("read_csv_auto('/tmp/it''s/x.csv', delim=',')")
    );
}
