//! Export request types: the format, the JSON layout and the sheets to write.

use serde::Deserialize;

use crate::error::{ApiError, ApiResult};

const MIN_SHEETS: usize = 1;
const MAX_SHEETS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Xlsx,
    Parquet,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonLayout {
    #[default]
    Rows,
    Columns,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSheet {
    pub node_id: String,
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub format: ExportFormat,
    #[serde(default)]
    pub json_layout: JsonLayout,
    #[serde(default)]
    pub filename: Option<String>,
    pub sheets: Vec<ExportSheet>,
}

impl ExportRequest {
    /// Rejects a request whose sheet count is outside 1 to 100.
    pub fn validate(&self) -> ApiResult<()> {
        if (MIN_SHEETS..=MAX_SHEETS).contains(&self.sheets.len()) {
            Ok(())
        } else {
            Err(ApiError::unprocessable(format!(
                "Export requires {MIN_SHEETS} to {MAX_SHEETS} sheets"
            )))
        }
    }
}
