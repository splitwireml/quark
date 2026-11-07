//! The request body that confirms a staged workbook.

use serde::Deserialize;

/// The sheets to import from a staged workbook.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbookConfirm {
    pub sheets: Vec<String>,
}
