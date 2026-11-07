//! The find request and its response.

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

const MAX_TERM_CHARS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchDirection {
    #[default]
    Next,
    Previous,
}

fn before_start() -> i64 {
    -1
}

/// Where to look and where the last match was: `after_row` and `after_column` of -1 start from the top.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellSearch {
    pub sql: String,
    pub term: String,
    pub columns: Vec<String>,
    #[serde(default = "before_start")]
    pub after_row: i64,
    #[serde(default = "before_start")]
    pub after_column: i64,
    #[serde(default)]
    pub direction: SearchDirection,
}

impl CellSearch {
    /// Rejects an empty or over-long term, no columns and a cursor before -1 with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        let term_chars = self.term.chars().count();
        if term_chars == 0 || term_chars > MAX_TERM_CHARS {
            return Err(ApiError::unprocessable(
                "Search term must be 1 to 1000 characters",
            ));
        }
        if self.columns.is_empty() {
            return Err(ApiError::unprocessable("Search needs at least one column"));
        }
        if self.after_row < -1 || self.after_column < -1 {
            return Err(ApiError::unprocessable("Invalid search position"));
        }
        Ok(())
    }
}

/// One matching cell: its 0-based row in the View, its column and that column's place in the request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundCell {
    pub row: i64,
    pub column: String,
    pub column_index: usize,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindResponse {
    #[serde(rename = "match")]
    pub found: Option<FoundCell>,
}
