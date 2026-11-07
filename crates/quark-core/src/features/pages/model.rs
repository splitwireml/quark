//! The page request's output format and the page's response types.

use serde::Serialize;
use serde_json::Value;

use crate::shared::columns::ColumnSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMeta {
    pub columns: Vec<ColumnSummary>,
    pub page: i64,
    pub page_size: i64,
    pub total_rows: u64,
    pub total_pages: u64,
    pub elapsed_ms: f64,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PageBody {
    Json(Value),
    Arrow(Vec<u8>),
}
