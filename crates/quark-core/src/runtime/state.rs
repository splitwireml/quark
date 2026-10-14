//! Shared app state: where the data lives, the columnar worker and the catalog lock.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use anyhow::Context;

use crate::error::ApiError;
use crate::runtime::cache::columnar::ColumnarWorker;
use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::catalog::Catalog;
use crate::runtime::dirs::Dirs;
use crate::runtime::files::{remove_cache_files, remove_orphan_uploads};
use crate::runtime::mount::import_job;
use crate::runtime::registry::{SourceRecord, default_project, load_projects, load_registry};

pub(crate) fn cannot_open(error: impl Display) -> ApiError {
    ApiError::bad_request(format!("Could not open source: {error}"))
}

pub(crate) fn registry_not_saved(error: io::Error) -> ApiError {
    tracing::error!(%error, "could not save registry");
    ApiError::internal("Internal error")
}

/// Python's global DuckDB error handler answers every engine failure with this 422.
pub(crate) fn invalid_value(error: impl Display) -> ApiError {
    ApiError::unprocessable(format!("Invalid filter value: {error}"))
}

pub struct Shared {
    pub dirs: Dirs,
    pub catalog: Mutex<Catalog>,
    /// Fills the columnar cache; `None` once the app has shut down.
    columnar: Mutex<Option<ColumnarWorker>>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Shared>);

impl AppState {
    /// Prepares the folders and reads the catalog. Never writes `projects.json` or `registry.json`.
    pub fn load(dirs: Dirs) -> anyhow::Result<Self> {
        let uploads = dirs.uploads();
        for folder in [&dirs.data, &uploads, &dirs.columnar()] {
            fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
        }
        let spill = dirs.spill();
        if spill.exists() {
            fs::remove_dir_all(&spill).context("emptying the spill folder")?;
        }
        fs::create_dir_all(&spill).context("creating the spill folder")?;
        let exports = dirs.exports();
        if exports.exists() {
            fs::remove_dir_all(&exports).context("emptying the exports folder")?;
        }
        fs::create_dir_all(&exports).context("creating the exports folder")?;

        let mut projects = vec![default_project()];
        projects.extend(load_projects(&dirs.projects_file()));
        let registry = load_registry(&dirs.registry_file());
        let sources = registry
            .iter()
            .filter_map(SourceRecord::from_raw)
            .filter(|source| source.source.is_file())
            .collect();
        remove_orphan_uploads(&uploads, &registry)?;

        let shared = Arc::new_cyclic(|weak: &Weak<Shared>| {
            let weak = weak.clone();
            let on_ready = Box::new(move |source_id: String, key: ColumnarKey| {
                if let Some(shared) = weak.upgrade() {
                    Self(shared).cache_ready(&source_id, &key);
                }
            });
            let worker = ColumnarWorker::start(&dirs.cache, &spill, on_ready);
            Shared {
                dirs,
                catalog: Mutex::new(Catalog {
                    projects,
                    registry,
                    sources,
                    engines: HashMap::new(),
                    remount: HashSet::new(),
                }),
                columnar: Mutex::new(Some(worker)),
            }
        });
        Ok(Self(shared))
    }

    /// Queues the source's columnar import unless its cache is already there.
    /// Repeats are cheap: the worker skips finished and failed imports.
    pub(crate) fn queue_import(&self, source: &SourceRecord) {
        let Some(job) = import_job(source) else {
            return;
        };
        if job.key.is_ready(&self.0.dirs.cache) {
            job.key.touch(&self.0.dirs.cache);
            return;
        }
        let columnar = self
            .0
            .columnar
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(worker) = columnar.as_ref() {
            worker.enqueue(job);
        }
    }

    /// Forgets the engines that mount `source_id`, so the next request mounts its new cache.
    /// Work already running finishes on the old engine. A source deleted during its import
    /// left a cache nobody needs: it goes, unless another source reads the same file.
    fn cache_ready(&self, source_id: &str, key: &ColumnarKey) {
        let engines = {
            let mut catalog = self.catalog();
            let Some(source) = catalog.sources.iter().find(|source| source.id == source_id) else {
                let is_orphan = !catalog.uses_cache(key);
                drop(catalog);
                if is_orphan {
                    remove_cache_files(key, &self.0.dirs.cache);
                }
                return;
            };
            let workspace = catalog
                .project(source.project())
                .ok()
                .map(|project| project.node_id.clone());
            let workspace = workspace.and_then(|node_id| {
                let engine = catalog.engines.remove(&node_id)?;
                catalog.remount.insert(node_id);
                Some(engine)
            });
            [catalog.engines.remove(source_id), workspace]
        };
        drop(engines);
    }

    /// Locks the catalog, recovering it if a panicking thread poisoned the mutex.
    pub fn catalog(&self) -> MutexGuard<'_, Catalog> {
        self.0
            .catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Stops the columnar worker, then drops every engine, closing its connection.
    pub fn shutdown(&self) {
        let worker = self
            .0
            .columnar
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(mut worker) = worker {
            worker.shutdown();
        }
        let engines = std::mem::take(&mut self.catalog().engines);
        drop(engines);
    }
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
