//! Request types shared by the data endpoints: connectors, filters, sorts and paging.

use serde::Deserialize;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};

const MAX_PAGE_SIZE: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Connector {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub column: String,
    pub operator: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub connector: Connector,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub column: String,
    pub direction: Direction,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    100
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl Default for QueryRequest {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
            filters: Vec::new(),
            sorts: Vec::new(),
            dedupe_columns: Vec::new(),
        }
    }
}

impl QueryRequest {
    /// Rejects `page < 1` and `page_size` outside `1..=1000` with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        if self.page < 1 || !(1..=MAX_PAGE_SIZE).contains(&self.page_size) {
            return Err(ApiError::unprocessable("Invalid page or page size"));
        }
        Ok(())
    }
}

/// A [`QueryRequest`] plus the SQL text. The fields are repeated because serde
/// cannot combine `flatten` with `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlQueryRequest {
    pub sql: String,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl SqlQueryRequest {
    pub fn query(&self) -> QueryRequest {
        QueryRequest {
            page: self.page,
            page_size: self.page_size,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe_columns.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
