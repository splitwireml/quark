//! Project rules: the name limit, the new project record and the listing with source counts.

use uuid::Uuid;

use super::model::ProjectSummary;
use crate::error::{ApiError, ApiResult};
use crate::runtime::AppState;
use crate::runtime::registry::ProjectRecord;

/// Longest raw project name, in Unicode scalar values.
const PROJECT_NAME_LIMIT: usize = 200;

/// Every project with its active source count, the default project first.
pub fn list_projects(state: &AppState) -> Vec<ProjectSummary> {
    let catalog = state.catalog();
    catalog
        .projects
        .iter()
        .map(|project| ProjectSummary {
            project: project.clone(),
            source_count: catalog.sources_of(&project.id).count(),
        })
        .collect()
}

/// Saves a new project; the raw name must hold 1 to 200 characters and not be blank.
pub fn create_project(state: &AppState, name: &str) -> ApiResult<ProjectSummary> {
    let trimmed = name.trim();
    if !(1..=PROJECT_NAME_LIMIT).contains(&name.chars().count()) || trimmed.is_empty() {
        return Err(ApiError::unprocessable("Project name is required"));
    }
    let id = Uuid::new_v4().simple().to_string();
    let project = state.save_project(ProjectRecord {
        node_id: format!("project_{id}"),
        id,
        name: trimmed.to_owned(),
    })?;
    Ok(ProjectSummary {
        project,
        source_count: 0,
    })
}
