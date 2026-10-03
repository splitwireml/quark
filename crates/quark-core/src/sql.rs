//! SQL quoting and DuckDB scan expressions.

use std::path::Path;

/// Quotes an identifier, doubling any inner `"`.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Quotes a string literal, doubling any inner `'`.
pub fn sql_string(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The DuckDB table function that reads `path`, chosen by its lowercase extension.
pub fn scan_expression(path: &str) -> Option<String> {
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    let file = sql_string(path);
    match extension.as_str() {
        "csv" => Some(format!("read_csv_auto({file}, delim=',')")),
        "tsv" => Some(format!(r"read_csv_auto({file}, delim='\t')")),
        "parquet" => Some(format!("read_parquet({file})")),
        "json" | "ndjson" | "jsonl" => Some(format!("read_json_auto({file})")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
}
