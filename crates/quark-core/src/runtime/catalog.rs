//! The catalog of projects and sources: its state, lookups and the saving of projects.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::engine::Engine;
use crate::runtime::mount::import_job;
use crate::runtime::registry::{
    DEFAULT_PROJECT_ID, ProjectRecord, RawRecord, SourceRecord, save_projects,
};
use crate::runtime::state::AppState;

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
}

impl AppState {
    /// Writes the project to `projects.json`, then adds it to the catalog.
    pub(crate) fn save_project(&self, project: ProjectRecord) -> ApiResult<ProjectRecord> {
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
        Ok(project)
    }
}
