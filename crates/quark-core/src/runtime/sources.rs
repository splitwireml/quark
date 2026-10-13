//! Registering sources, opening their engines and deleting them with their files.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::runtime::engine::Engine;
use crate::runtime::files::{remove_cache_files, remove_with_retry};
use crate::runtime::mount::import_job;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, RawRecord, SourceRecord, save_registry};
use crate::runtime::state::{AppState, cannot_open, registry_not_saved};

impl AppState {
    /// Where an upload with this id and extension (with its dot) is stored.
    pub fn upload_path(&self, node_id: &str, ext: &str) -> PathBuf {
        self.0.dirs.uploads().join(format!("{node_id}{ext}"))
    }

    /// Opens the engine behind `source`; a file that cannot be read is a 400.
    pub(crate) fn open_node_engine(&self, source: &SourceRecord) -> ApiResult<Engine> {
        Engine::open_node(source, &self.0.dirs).map_err(|error| cannot_open(format!("{error:#}")))
    }

    /// Adds an opened source to its project: the registry entry is saved before the catalog changes.
    pub(crate) fn register_source(
        &self,
        source: SourceRecord,
        record: RawRecord,
        engine: Engine,
    ) -> ApiResult<()> {
        let mut catalog = self.catalog();
        let workspace_id = catalog.project(source.project())?.node_id.clone();
        catalog.registry.push(record);
        if let Err(error) = save_registry(&self.0.dirs.registry_file(), &catalog.registry) {
            catalog.registry.pop();
            return Err(registry_not_saved(error));
        }
        catalog.engines.remove(&workspace_id);
        if source.project() == DEFAULT_PROJECT_ID {
            catalog.engines.insert(source.id.clone(), Arc::new(engine));
        }
        self.queue_import(&source);
        catalog.sources.push(source);
        Ok(())
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
