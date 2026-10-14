//! Source rules: what a project lists, which files an upload may be, and how an upload is recorded.

use std::path::Path;

use super::model::{DatasetEntry, LegacyNode, SourceDetail, SourceSummary};
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::{datasets as describe_datasets, describe};
use crate::runtime::AppState;
use crate::runtime::cancel::Cancelled;
use crate::runtime::mount::ViewInfo;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, SourceRecord};
use crate::runtime::state::invalid_value;
use crate::shared::naming::dataset_name;
use crate::shared::sql::quote_ident;

pub fn project_sources(state: &AppState, project_id: &str) -> ApiResult<Vec<SourceSummary>> {
    let catalog = state.catalog();
    catalog.project(project_id)?;
    Ok(catalog
        .sources_of(project_id)
        .map(|source| SourceSummary {
            id: source.id.clone(),
            name: source.name.clone(),
        })
        .collect())
}

pub fn source_detail(
    state: &AppState,
    project_id: &str,
    source_id: &str,
) -> ApiResult<SourceDetail> {
    let source = {
        let catalog = state.catalog();
        catalog.project(project_id)?;
        catalog
            .sources_of(project_id)
            .find(|source| source.id == source_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Source not found"))?
    };
    let views = state.mount_sources(project_id, std::slice::from_ref(&source))?;
    Ok(SourceDetail {
        id: source.id,
        name: source.name,
        kind: source.kind,
        project_id: project_id.to_owned(),
        views,
    })
}

/// Every view of the project, mounting its sources in registry order.
pub fn project_views(state: &AppState, project_id: &str) -> ApiResult<Vec<ViewInfo>> {
    let sources: Vec<SourceRecord> = {
        let catalog = state.catalog();
        catalog.project(project_id)?;
        catalog.sources_of(project_id).cloned().collect()
    };
    state.mount_sources(project_id, &sources)
}

/// Active sources of the default project, whose ids double as node ids.
pub fn legacy_nodes(state: &AppState) -> Vec<LegacyNode> {
    state
        .catalog()
        .sources_of(DEFAULT_PROJECT_ID)
        .map(|source| LegacyNode {
            id: source.id.clone(),
            name: source.name.clone(),
            kind: source.kind.clone(),
            source: source.source.to_string_lossy().into_owned(),
        })
        .collect()
}

/// Every table and view of the node's engine, with column names.
pub fn datasets(state: &AppState, node_id: &str) -> ApiResult<Vec<DatasetEntry>> {
    let engine = state.engine_for_node(node_id)?;
    engine
        .read(None, |conn| {
            describe_datasets(conn)
                .map_err(invalid_value)?
                .into_iter()
                .map(|info| {
                    let table =
                        format!("{}.{}", quote_ident(&info.schema), quote_ident(&info.name));
                    let columns = describe(conn, &table, &[])
                        .map_err(invalid_value)?
                        .into_iter()
                        .map(|column| column.name)
                        .collect();
                    Ok(DatasetEntry { info, columns })
                })
                .collect()
        })
        .map_err(|Cancelled| ApiError::internal("Internal error"))?
}

/// The lowercase extension with its dot, if the desktop build can read that kind of file.
pub(crate) fn supported_extension(name: &str) -> ApiResult<String> {
    let ext = Path::new(name)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    match ext.as_str() {
        ".csv" | ".tsv" | ".parquet" | ".json" | ".ndjson" | ".jsonl" | ".xlsx" | ".duckdb"
        | ".db" => Ok(ext),
        _ => Err(ApiError::bad_request("Unsupported file type")),
    }
}

/// What follows the last `/` or `\` in the file name a client sent.
pub(crate) fn uploaded_name(file_name: &str) -> &str {
    file_name.rsplit(['/', '\\']).next().unwrap_or(file_name)
}

/// The source an upload saved at `path` becomes; a database keeps its own dataset names.
pub(crate) fn upload_source(
    project_id: Option<&str>,
    node_id: &str,
    original_name: &str,
    path: &Path,
) -> SourceRecord {
    let name = Path::new(original_name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_database = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("duckdb") || ext.eq_ignore_ascii_case("db"));
    SourceRecord {
        id: node_id.to_owned(),
        name,
        kind: "upload".to_owned(),
        source: path.to_owned(),
        project_id: project_id.map(str::to_owned),
        dataset_name: (!is_database).then(|| dataset_name(original_name)),
        sheets: None,
    }
}
