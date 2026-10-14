//! Classifies DuckDB column type names as numeric, text, date or natively Arrow-encodable.

const NUMERIC: [&str; 14] = [
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "HUGEINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "UHUGEINT",
    "FLOAT",
    "REAL",
    "DOUBLE",
    "DECIMAL",
];
const INTEGER: [&str; 10] = [
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "HUGEINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "UHUGEINT",
];
const TEXT: [&str; 3] = ["VARCHAR", "CHAR", "TEXT"];
const DATE: [&str; 3] = ["DATE", "TIME", "TIMESTAMP"];
const NATIVE: [&str; 14] = [
    "BOOLEAN",
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "FLOAT",
    "REAL",
    "DOUBLE",
    "VARCHAR",
    "BLOB",
];

/// True when the uppercased type starts with one of the prefixes.
fn has_prefix(type_: &str, prefixes: &[&str]) -> bool {
    let upper = type_.to_ascii_uppercase();
    prefixes.iter().any(|prefix| upper.starts_with(prefix))
}

pub fn is_numeric(type_: &str) -> bool {
    has_prefix(type_, &NUMERIC)
}

pub fn is_integer(type_: &str) -> bool {
    has_prefix(type_, &INTEGER)
}

pub fn is_text(type_: &str) -> bool {
    has_prefix(type_, &TEXT)
}

pub fn profile_kind(type_: &str) -> Option<&'static str> {
    let upper = type_.to_ascii_uppercase();
    if is_numeric(type_) {
        Some("numeric")
    } else if is_text(type_) || upper.starts_with("ENUM") || upper == "BOOLEAN" {
        Some("categorical")
    } else if has_prefix(type_, &DATE) {
        Some("date")
    } else {
        None
    }
}

pub fn is_native(type_: &str) -> bool {
    NATIVE.contains(&type_.to_ascii_uppercase().as_str())
}

#[cfg(test)]
mod tests;
