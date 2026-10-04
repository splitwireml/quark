//! Classification of DuckDB column types, mirroring the Python backend.

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
mod tests {
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
}
