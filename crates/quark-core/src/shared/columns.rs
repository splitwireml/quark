//! Column descriptions shared by queries, pages and caches: a column's name and type, and its profile summary.

use serde::Serialize;

/// A column's name and DuckDB type, as reported in response metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub numeric: bool,
    pub profile_kind: Option<&'static str>,
    pub null_fraction: f64,
}
