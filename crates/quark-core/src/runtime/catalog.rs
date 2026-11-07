//! The catalog of projects and sources: its state, lookups and the saving of projects.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use crate::error::{ApiError, ApiResult};
use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::cache::workbook::sheet_names;
use crate::runtime::engine::Engine;
use crate::runtime::files::remove_with_retry;
use crate::runtime::joins::JoinSlots;
use crate::runtime::mount::import_job;
use crate::runtime::registry::{
    DEFAULT_PROJECT_ID, ProjectRecord, RawRecord, SourceRecord, save_projects,
};
use crate::runtime::state::{AppState, cannot_open};

/// An uploaded workbook whose sheets are not yet chosen. It lives in memory only.
#[derive(Debug, Clone)]
pub struct PendingWorkbook {
    pub id: String,
    pub name: String,
    pub source: PathBuf,
    pub sheets: Vec<String>,
    pub project_id: Option<String>,
}

impl PendingWorkbook {
    fn project(&self) -> &str {
        self.project_id.as_deref().unwrap_or(DEFAULT_PROJECT_ID)
    }

    fn to_public(&self) -> Value {
        let mut public =
            json!({"id": self.id, "name": self.name, "kind": "workbook", "sheets": self.sheets});
        if let Some(project) = &self.project_id {
            public["project_id"] = json!(project);
        }
        public
    }
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
    /// Cross-source join engines: reachable by id, never listed.
    pub(crate) joins: JoinSlots,
    /// Workspaces dropped for a new cache: their replacement mounts the whole project.
    pub(crate) remount: HashSet<String>,
    /// Uploaded workbooks waiting for a sheet selection, by stage id.
    pub(crate) pending_workbooks: HashMap<String, PendingWorkbook>,
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

    /// Lists the sheets of the workbook saved at `path` and holds it until they are confirmed.
    /// A file that cannot be read is deleted and answers 400.
    pub(crate) fn stage_workbook(
        &self,
        project_id: Option<&str>,
        id: &str,
        name: &str,
        path: &Path,
    ) -> ApiResult<Value> {
        self.catalog()
            .project(project_id.unwrap_or(DEFAULT_PROJECT_ID))?;
        let sheets = sheet_names(path).map_err(|error| {
            remove_with_retry(path);
            cannot_open(format!("{error:#}"))
        })?;
        let stage = PendingWorkbook {
            id: id.to_owned(),
            name: name.to_owned(),
            source: path.to_owned(),
            sheets,
            project_id: project_id.map(str::to_owned),
        };
        let public = stage.to_public();
        self.catalog()
            .pending_workbooks
            .insert(id.to_owned(), stage);
        Ok(public)
    }

    /// Removes the stage from the pending ones; a stage of another project is not found.
    pub(crate) fn take_pending_workbook(
        &self,
        stage_id: &str,
        project_id: Option<&str>,
    ) -> ApiResult<PendingWorkbook> {
        let project = project_id.unwrap_or(DEFAULT_PROJECT_ID);
        let mut catalog = self.catalog();
        catalog.project(project)?;
        match catalog.pending_workbooks.get(stage_id) {
            Some(stage) if stage.project() == project => {}
            _ => return Err(ApiError::not_found("Workbook upload not found")),
        }
        catalog
            .pending_workbooks
            .remove(stage_id)
            .ok_or_else(|| ApiError::not_found("Workbook upload not found"))
    }

    /// Puts a taken stage back, so a failed confirmation can be retried.
    pub(crate) fn restore_pending_workbook(&self, stage: PendingWorkbook) {
        self.catalog()
            .pending_workbooks
            .insert(stage.id.clone(), stage);
    }

    /// Forgets the stage and deletes its file.
    pub(crate) fn discard_pending_workbook(
        &self,
        stage_id: &str,
        project_id: Option<&str>,
    ) -> ApiResult<()> {
        let stage = self.take_pending_workbook(stage_id, project_id)?;
        remove_with_retry(&stage.source);
        Ok(())
    }
}
