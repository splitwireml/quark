//! Dataset, view and schema identifiers that match the Python backend byte for byte.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::fmt::Write;

/// Compact JSON the way Python's `json.dumps(separators=(",", ":"))` writes it:
/// every non-ASCII character (and DEL) becomes a lowercase `\uXXXX` escape.
pub fn python_json(value: &Value) -> String {
    let compact = value.to_string();
    let mut out = String::with_capacity(compact.len());
    for ch in compact.chars() {
        if ch.is_ascii() && ch != '\u{7f}' {
            out.push(ch);
        } else {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out
}

/// URL-safe base64 without padding.
pub fn b64(text: &str) -> String {
    URL_SAFE_NO_PAD.encode(text)
}

pub fn dataset_id(schema: &str, name: &str) -> String {
    b64(&python_json(&json!([schema, name])))
}

pub fn view_id(project_id: &str, source_id: &str, dataset_id: &str) -> String {
    b64(&python_json(&json!([project_id, source_id, dataset_id])))
}

pub fn source_schema(source_id: &str, dataset_id: &str) -> String {
    format!(
        "source_{}",
        b64(&python_json(&json!([source_id, dataset_id])))
    )
}

pub fn database_alias(source_id: &str) -> String {
    format!("database_{}", b64(source_id))
}

#[cfg(test)]
mod tests {
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
}
