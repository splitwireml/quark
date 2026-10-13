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
mod tests;
