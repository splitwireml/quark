//! Opening engines on demand: a node's own engine, a project's workspace and its mounted sources.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::error::{ApiError, ApiResult};
use crate::relation::describe::{DatasetInfo, describe};
use crate::runtime::catalog::Catalog;
use crate::runtime::engine::Engine;
use crate::runtime::mount::ViewInfo;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, ProjectRecord, SourceRecord};
use crate::runtime::state::{AppState, invalid_value};
use crate::shared::sql::quote_ident;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetEntry {
    #[serde(flatten)]
    pub info: DatasetInfo,
    pub columns: Vec<String>,
}

impl AppState {
    /// The engine behind a project node id or an active default-project source id.
    pub fn engine_for_node(&self, node_id: &str) -> ApiResult<Arc<Engine>> {
        let mut catalog = self.catalog();
        if let Some(project) = catalog
            .projects
            .iter()
            .find(|project| project.node_id == node_id)
            .cloned()
        {
            return self.workspace_in(&mut catalog, &project);
        }
        let source = catalog
            .sources_of(DEFAULT_PROJECT_ID)
            .find(|source| source.id == node_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Node not found"))?;
        if let Some(engine) = catalog.engines.get(node_id) {
            return Ok(Arc::clone(engine));
        }
        let generation = self.0.next_generation.fetch_add(1, Ordering::Relaxed);
        let engine = Engine::open_node(&source, &self.0.dirs, generation)
            .map_err(|error| ApiError::bad_request(format!("Could not open source: {error:#}")))?;
        let engine = Arc::new(engine);
        catalog
            .engines
            .insert(node_id.to_owned(), Arc::clone(&engine));
        self.queue_import(&source);
        Ok(engine)
    }

    /// Every table and view of the node's engine, with column names.
    pub fn datasets(&self, node_id: &str) -> ApiResult<Vec<DatasetEntry>> {
        let engine = self.engine_for_node(node_id)?;
        let inner = engine.lock();
        crate::relation::describe::datasets(&inner.conn)
            .map_err(invalid_value)?
            .into_iter()
            .map(|info| {
                let table = format!("{}.{}", quote_ident(&info.schema), quote_ident(&info.name));
                let columns = describe(&inner.conn, &table, &[])
                    .map_err(invalid_value)?
                    .into_iter()
                    .map(|column| column.name)
                    .collect();
                Ok(DatasetEntry { info, columns })
            })
            .collect()
    }

    /// The project's workspace engine, opened on first use. The caller holds the catalog lock.
    fn workspace_in(
        &self,
        catalog: &mut Catalog,
        project: &ProjectRecord,
    ) -> ApiResult<Arc<Engine>> {
        if let Some(engine) = catalog.engines.get(&project.node_id) {
            return Ok(Arc::clone(engine));
        }
        let sources: Vec<SourceRecord> = catalog.sources_of(&project.id).cloned().collect();
        let generation = self.0.next_generation.fetch_add(1, Ordering::Relaxed);
        let engine = Engine::open_workspace(project, &sources, &self.0.dirs, generation)
            .map_err(invalid_value)?;
        if catalog.remount.contains(&project.node_id) {
            for source in &sources {
                engine.mount(project, source).map_err(invalid_value)?;
            }
            catalog.remount.remove(&project.node_id);
        }
        let engine = Arc::new(engine);
        catalog
            .engines
            .insert(project.node_id.clone(), Arc::clone(&engine));
        Ok(engine)
    }

    /// Mounts `sources` into the project's workspace under the engine lock only.
    /// A failed mount may leave half-made schemas, so the workspace is dropped.
    pub(crate) fn mount_sources(
        &self,
        project_id: &str,
        sources: &[SourceRecord],
    ) -> ApiResult<Vec<ViewInfo>> {
        let (project, engine) = {
            let mut catalog = self.catalog();
            let project = catalog.project(project_id)?.clone();
            let engine = self.workspace_in(&mut catalog, &project)?;
            (project, engine)
        };
        let mut views = Vec::new();
        for source in sources {
            match engine.mount(&project, source) {
                Ok(mounted) => {
                    views.extend(mounted);
                    self.queue_import(source);
                }
                Err(error) => {
                    let mut catalog = self.catalog();
                    if catalog
                        .engines
                        .get(&project.node_id)
                        .is_some_and(|current| Arc::ptr_eq(current, &engine))
                    {
                        catalog.engines.remove(&project.node_id);
                    }
                    return Err(invalid_value(error));
                }
            }
        }
        Ok(views)
    }
}

#[cfg(test)]
mod tests;
