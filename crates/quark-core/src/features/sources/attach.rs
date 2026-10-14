//! Attaching a DuckDB file by path and reporting where a source lives on disk.

use std::env;
use std::fs;
use std::path::{Component, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::relation::describe::datasets;
use crate::runtime::AppState;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, SourceRecord};
use crate::runtime::state::cannot_open;

use super::service::{public_source, source_record};

/// The body of both attach routes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachRequest {
    pub path: String,
}

/// Registers the DuckDB file at `request.path` as a read-only source of the project,
/// or of the default project on the legacy route (`project_id` is `None`).
pub fn attach_database(
    state: &AppState,
    project_id: Option<&str>,
    request: &AttachRequest,
) -> ApiResult<Value> {
    state
        .catalog()
        .project(project_id.unwrap_or(DEFAULT_PROJECT_ID))?;
    let path = expand_path(&request.path);
    let is_database = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("duckdb") || ext.eq_ignore_ascii_case("db"));
    if !is_database {
        return Err(ApiError::bad_request(
            "Only .duckdb and .db paths can be attached",
        ));
    }
    if !path.is_file() {
        return Err(ApiError::not_found("Database not found"));
    }
    let source = SourceRecord {
        id: Uuid::new_v4().simple().to_string(),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind: "attached".to_owned(),
        source: path,
        project_id: project_id.map(str::to_owned),
        dataset_name: None,
        sheets: None,
    };
    let engine = state.open_node_engine(&source)?;
    datasets(&engine.lock().conn).map_err(cannot_open)?;
    let public = public_source(&source);
    let record = source_record(&source);
    state.register_source(source, record, engine)?;
    Ok(public)
}

/// The absolute path of a source's file, as `{path}`.
pub fn source_path(state: &AppState, project_id: &str, source_id: &str) -> ApiResult<Value> {
    let catalog = state.catalog();
    catalog.project(project_id)?;
    let source = catalog
        .sources_of(project_id)
        .find(|source| source.id == source_id)
        .ok_or_else(|| ApiError::not_found("Source not found"))?;
    let path = expand_path(&source.source.to_string_lossy());
    Ok(json!({"path": path.to_string_lossy()}))
}

/// Resolves a path as Python's `expanduser().resolve()` does: expands a leading `~`, makes
/// the path absolute, follows symlinks through the longest existing ancestor and appends
/// the rest lexically (`..` pops). The path need not exist and its case is never changed.
pub(crate) fn expand_path(raw: &str) -> PathBuf {
    let expanded = match raw.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => match home_folder() {
            Some(home) => home.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(raw),
        },
        _ => PathBuf::from(raw),
    };
    let absolute = std::path::absolute(&expanded).unwrap_or(expanded);
    let components: Vec<Component> = absolute.components().collect();
    let Some((mut resolved, rest)) = (1..=components.len()).rev().find_map(|count| {
        let ancestor: PathBuf = components[..count].iter().collect();
        let real = fs::canonicalize(ancestor).ok()?;
        Some((without_verbatim_prefix(real), &components[count..]))
    }) else {
        return absolute;
    };
    for component in rest {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    resolved
}

/// Turns a Windows verbatim path (`\\?\C:\x`, `\\?\UNC\host\share`) back into its plain form.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let plain = path
        .to_str()
        .and_then(|text| match text.strip_prefix(r"\\?\UNC\") {
            Some(unc) => Some(format!(r"\\{unc}")),
            None => text.strip_prefix(r"\\?\").map(str::to_owned),
        });
    plain.map_or(path, PathBuf::from)
}

fn home_folder() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}
