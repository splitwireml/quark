//! Workbook rules: checking the chosen sheets and registering the confirmed workbook as a source.

use std::collections::HashSet;

use serde_json::Value;

use super::model::WorkbookConfirm;
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::datasets;
use crate::runtime::AppState;
use crate::runtime::catalog::PendingWorkbook;
use crate::runtime::registry::SourceRecord;
use crate::runtime::state::cannot_open;

/// Registers the staged workbook with the chosen sheets and answers with the new node.
/// The stage stays when the selection is wrong or the workbook cannot be opened.
pub fn confirm_workbook(
    state: &AppState,
    project_id: Option<&str>,
    stage_id: &str,
    request: &WorkbookConfirm,
) -> ApiResult<Value> {
    let stage = state.take_pending_workbook(stage_id, project_id)?;
    let result = register(state, &stage, &request.sheets);
    if result.is_err() {
        state.restore_pending_workbook(stage);
    }
    result
}

fn register(state: &AppState, stage: &PendingWorkbook, sheets: &[String]) -> ApiResult<Value> {
    let unique: HashSet<&String> = sheets.iter().collect();
    if sheets.is_empty()
        || unique.len() != sheets.len()
        || !sheets.iter().all(|sheet| stage.sheets.contains(sheet))
    {
        return Err(ApiError::unprocessable(
            "Select one or more unique workbook sheets",
        ));
    }
    let source = SourceRecord {
        id: stage.id.clone(),
        name: stage.name.clone(),
        kind: "upload".to_owned(),
        source: stage.source.clone(),
        project_id: stage.project_id.clone(),
        dataset_name: None,
        sheets: Some(sheets.to_vec()),
    };
    let engine = state.open_node_engine(&source)?;
    datasets(&engine.primary().conn).map_err(cannot_open)?;
    let public = source.to_public();
    let record = source.to_raw();
    state.register_source(source, record, engine)?;
    Ok(public)
}

/// Forgets the staged workbook and deletes its file.
pub fn discard_workbook(
    state: &AppState,
    project_id: Option<&str>,
    stage_id: &str,
) -> ApiResult<()> {
    state.discard_pending_workbook(stage_id, project_id)
}
