//! The catalog of projects and sources, and the methods that list them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::engine::Engine;
use crate::runtime::mount::{ViewInfo, import_job};
use crate::runtime::registry::{
    DEFAULT_PROJECT_ID, ProjectRecord, RawRecord, SourceRecord, save_projects,
};
use crate::runtime::state::AppState;

/// Longest raw project name, in Unicode scalar values.
const PROJECT_NAME_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: ProjectRecord,
    pub source_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceDetail {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub project_id: String,
    pub views: Vec<ViewInfo>,
}

/// An active source of the default project, as listed by `GET /nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: String,
}

pub struct Catalog {
    /// The default project first, then the saved ones.
    pub projects: Vec<ProjectRecord>,
    /// Every stored entry, including inactive ones and unknown keys.
    pub registry: Vec<RawRecord>,
    /// The entries whose file exists, in registry order.
    pub sources: Vec<SourceRecord>,
    /// Opened engines by node id; nothing is opened at startup.
    pub engines: HashMap<String, Arc<Engine>>,
    /// Workspaces dropped for a new cache: their replacement mounts the whole project.
    pub(crate) remount: HashSet<String>,
}

impl Catalog {
    pub(crate) fn project(&self, id: &str) -> ApiResult<&ProjectRecord> {
        self.projects
            .iter()
            .find(|project| project.id == id)
            .ok_or_else(|| ApiError::not_found("Project not found"))
    }

    /// The project's active sources in registry order.
    pub(crate) fn sources_of<'a>(
        &'a self,
        project_id: &'a str,
    ) -> impl Iterator<Item = &'a SourceRecord> {
        self.sources
            .iter()
            .filter(move |source| source.project() == project_id)
    }

    /// Whether an active source reads the cache file `key` names.
    pub(crate) fn uses_cache(&self, key: &ColumnarKey) -> bool {
        self.sources
            .iter()
            .filter_map(import_job)
            .any(|job| job.key == *key)
    }

    fn summary(&self, project: &ProjectRecord) -> ProjectSummary {
        ProjectSummary {
            project: project.clone(),
            source_count: self.sources_of(&project.id).count(),
        }
    }
}

impl AppState {
    /// Every project with its active source count, the default project first.
    pub fn projects(&self) -> Vec<ProjectSummary> {
        let catalog = self.catalog();
        catalog
            .projects
            .iter()
            .map(|project| catalog.summary(project))
            .collect()
    }

    /// Saves a new project; the raw name must hold 1 to 200 characters and not be blank.
    pub fn create_project(&self, name: &str) -> ApiResult<ProjectSummary> {
        let trimmed = name.trim();
        if !(1..=PROJECT_NAME_LIMIT).contains(&name.chars().count()) || trimmed.is_empty() {
            return Err(ApiError::unprocessable("Project name is required"));
        }
        let id = Uuid::new_v4().simple().to_string();
        let project = ProjectRecord {
            node_id: format!("project_{id}"),
            id,
            name: trimmed.to_owned(),
        };
        let mut catalog = self.catalog();
        let mut saved: Vec<ProjectRecord> = catalog
            .projects
            .iter()
            .filter(|saved| saved.id != DEFAULT_PROJECT_ID)
            .cloned()
            .collect();
        saved.push(project.clone());
        save_projects(&self.0.dirs.projects_file(), &saved).map_err(|error| {
            tracing::error!(%error, "could not save projects");
            ApiError::internal("Internal error")
        })?;
        catalog.projects.push(project.clone());
        Ok(catalog.summary(&project))
    }

    pub fn project_sources(&self, project_id: &str) -> ApiResult<Vec<SourceSummary>> {
        let catalog = self.catalog();
        catalog.project(project_id)?;
        Ok(catalog
            .sources_of(project_id)
            .map(|source| SourceSummary {
                id: source.id.clone(),
                name: source.name.clone(),
            })
            .collect())
    }

    pub fn source_detail(&self, project_id: &str, source_id: &str) -> ApiResult<SourceDetail> {
        let source = {
            let catalog = self.catalog();
            catalog.project(project_id)?;
            catalog
                .sources_of(project_id)
                .find(|source| source.id == source_id)
                .cloned()
                .ok_or_else(|| ApiError::not_found("Source not found"))?
        };
        let views = self.mount_sources(project_id, std::slice::from_ref(&source))?;
        Ok(SourceDetail {
            id: source.id,
            name: source.name,
            kind: source.kind,
            project_id: project_id.to_owned(),
            views,
        })
    }

    /// Every view of the project, mounting its sources in registry order.
    pub fn project_views(&self, project_id: &str) -> ApiResult<Vec<ViewInfo>> {
        let sources: Vec<SourceRecord> = {
            let catalog = self.catalog();
            catalog.project(project_id)?;
            catalog.sources_of(project_id).cloned().collect()
        };
        self.mount_sources(project_id, &sources)
    }

    /// Active sources of the default project, whose ids double as node ids.
    pub fn legacy_nodes(&self) -> Vec<LegacyNode> {
        self.catalog()
            .sources_of(DEFAULT_PROJECT_ID)
            .map(|source| LegacyNode {
                id: source.id.clone(),
                name: source.name.clone(),
                kind: source.kind.clone(),
                source: source.source.to_string_lossy().into_owned(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
