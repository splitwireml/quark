//! Adding uploads to a project and deleting sources with their files.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use crate::error::{ApiError, ApiResult};
use crate::runtime::engine::Engine;
use crate::runtime::files::{remove_cache_files, remove_with_retry};
use crate::runtime::mount::import_job;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, RawRecord, SourceRecord, save_registry};
use crate::runtime::state::{AppState, cannot_open, registry_not_saved};
use crate::shared::naming::dataset_name;

impl AppState {
    /// Where an upload with this id and extension (with its dot) is stored.
    pub fn upload_path(&self, node_id: &str, ext: &str) -> PathBuf {
        self.0.dirs.uploads().join(format!("{node_id}{ext}"))
    }

    /// Adds the saved upload at `path` to the catalog, or deletes the file when that fails.
    pub fn register_upload(
        &self,
        project_id: Option<&str>,
        node_id: &str,
        original_name: &str,
        path: &Path,
    ) -> ApiResult<Value> {
        let result = self.add_upload(project_id, node_id, original_name, path);
        if result.is_err() {
            remove_with_retry(path);
        }
        result
    }

    fn add_upload(
        &self,
        project_id: Option<&str>,
        node_id: &str,
        original_name: &str,
        path: &Path,
    ) -> ApiResult<Value> {
        let workspace_id = self
            .catalog()
            .project(project_id.unwrap_or(DEFAULT_PROJECT_ID))?
            .node_id
            .clone();
        let name = Path::new(original_name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_database = path.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("duckdb") || ext.eq_ignore_ascii_case("db")
        });
        let source = SourceRecord {
            id: node_id.to_owned(),
            name,
            kind: "upload".to_owned(),
            source: path.to_owned(),
            project_id: project_id.map(str::to_owned),
            dataset_name: (!is_database).then(|| dataset_name(original_name)),
            sheets: None,
        };

        let generation = self.0.next_generation.fetch_add(1, Ordering::Relaxed);
        let engine = Engine::open_node(&source, &self.0.dirs, generation)
            .map_err(|error| cannot_open(format!("{error:#}")))?;
        crate::relation::describe::datasets(&engine.lock().conn).map_err(cannot_open)?;

        let mut record = RawRecord::new();
        record.insert("id".into(), source.id.clone().into());
        record.insert("name".into(), source.name.clone().into());
        record.insert("kind".into(), source.kind.clone().into());
        record.insert("source".into(), path.to_string_lossy().into());
        if let Some(project) = project_id {
            record.insert("project_id".into(), project.into());
        }
        if let Some(dataset) = &source.dataset_name {
            record.insert("dataset_name".into(), dataset.clone().into());
        }
        let public = match project_id {
            Some(project) => {
                json!({"id": node_id, "name": source.name, "kind": "upload", "project_id": project})
            }
            None => {
                json!({"id": node_id, "name": source.name, "kind": "upload", "source": path.to_string_lossy()})
            }
        };

        let mut catalog = self.catalog();
        catalog.registry.push(record);
        if let Err(error) = save_registry(&self.0.dirs.registry_file(), &catalog.registry) {
            catalog.registry.pop();
            return Err(registry_not_saved(error));
        }
        catalog.engines.remove(&workspace_id);
        if source.project() == DEFAULT_PROJECT_ID {
            catalog.engines.insert(node_id.to_owned(), Arc::new(engine));
        }
        self.queue_import(&source);
        catalog.sources.push(source);
        Ok(public)
    }

    /// Removes a source from its project. The engines go first so Windows lets the file go.
    pub fn delete_source(&self, project_id: &str, node_id: &str) -> ApiResult<()> {
        let (source, engines, cache) = {
            let mut catalog = self.catalog();
            let workspace_id = catalog.project(project_id)?.node_id.clone();
            let index = catalog
                .sources
                .iter()
                .position(|source| source.id == node_id && source.project() == project_id)
                .ok_or_else(|| ApiError::not_found("Node not found"))?;
            let source = catalog.sources.remove(index);
            let engines = [
                catalog.engines.remove(node_id),
                catalog.engines.remove(&workspace_id),
            ];
            // Sources over the same file share one cache file: it stays while any of them is left.
            let cache = import_job(&source)
                .map(|job| job.key)
                .filter(|key| !catalog.uses_cache(key));
            (source, engines, cache)
        };
        drop(engines);
        if let Some(key) = cache {
            remove_cache_files(&key, &self.0.dirs.cache);
        }
        if source.kind == "upload" {
            remove_with_retry(&source.source);
        }
        let mut catalog = self.catalog();
        catalog
            .registry
            .retain(|record| record.get("id").and_then(Value::as_str) != Some(node_id));
        save_registry(&self.0.dirs.registry_file(), &catalog.registry).map_err(registry_not_saved)
    }
}

#[cfg(test)]
mod tests;
