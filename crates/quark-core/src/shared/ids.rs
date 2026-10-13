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
mod tests;
