//! Opening engines on demand: a node's own engine, a project's workspace and its mounted sources.

use std::sync::Arc;
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::{cell::RefCell, rc::Rc};

use crate::error::{ApiError, ApiResult};
use crate::runtime::catalog::Catalog;
use crate::runtime::engine::Engine;
use crate::runtime::mount::ViewInfo;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, ProjectRecord, SourceRecord};
use crate::runtime::state::{AppState, invalid_value};

/// Attempts to open an engine outside the catalog lock before opening it under the lock.
const OPEN_ATTEMPTS: usize = 8;

/// What the catalog said a missing engine needs, copied so the open can run without the lock.
enum OpenPlan {
    Node(SourceRecord),
    Workspace {
        project: ProjectRecord,
        sources: Vec<SourceRecord>,
        is_remount: bool,
    },
}

enum Snapshot {
    Ready(Arc<Engine>),
    Open(OpenPlan),
}

impl OpenPlan {
    /// The engine map key this plan's engine is published under.
    fn key(&self) -> &str {
        match self {
            Self::Node(source) => &source.id,
            Self::Workspace { project, .. } => &project.node_id,
        }
    }

    /// Whether the catalog still holds the node and sources this plan was made from.
    fn is_current(&self, catalog: &Catalog) -> bool {
        match self {
            Self::Node(source) => catalog
                .sources_of(DEFAULT_PROJECT_ID)
                .any(|current| current.id == source.id),
            Self::Workspace {
                project, sources, ..
            } => {
                catalog
                    .projects
                    .iter()
                    .any(|current| current.id == project.id && current.node_id == project.node_id)
                    && catalog
                        .sources_of(&project.id)
                        .map(|current| &current.id)
                        .eq(sources.iter().map(|source| &source.id))
            }
        }
    }

    /// Stores the opened engine; a workspace that was remounted no longer needs it.
    fn insert(&self, catalog: &mut Catalog, engine: &Arc<Engine>) {
        catalog
            .engines
            .insert(self.key().to_owned(), Arc::clone(engine));
        if let Self::Workspace {
            project,
            is_remount: true,
            ..
        } = self
        {
            catalog.remount.remove(&project.node_id);
        }
    }
}

impl AppState {
    /// The engine behind a project node id or an active default-project source id.
    /// Engines open outside the catalog lock; the catalog only decides what to open and
    /// whether the result is still wanted.
    pub fn engine_for_node(&self, node_id: &str) -> ApiResult<Arc<Engine>> {
        for _ in 0..OPEN_ATTEMPTS {
            let plan = match snapshot(&self.catalog(), node_id)? {
                Snapshot::Ready(engine) => return Ok(engine),
                Snapshot::Open(plan) => plan,
            };
            before_open();
            let opened = self.open_plan(&plan);
            let mut catalog = self.catalog();
            let engine = match opened {
                Ok(engine) => Arc::new(engine),
                // The sources it read may be gone: look again before reporting the failure.
                Err(error) if plan.is_current(&catalog) => return Err(error),
                Err(_) => continue,
            };
            if let Some(existing) = catalog.engines.get(plan.key()) {
                return Ok(Arc::clone(existing));
            }
            if plan.is_current(&catalog) {
                plan.insert(&mut catalog, &engine);
                drop(catalog);
                self.queue_node_import(&plan);
                return Ok(engine);
            }
        }
        // The catalog keeps changing: open under the lock so the call still ends.
        let mut catalog = self.catalog();
        let plan = match snapshot(&catalog, node_id)? {
            Snapshot::Ready(engine) => return Ok(engine),
            Snapshot::Open(plan) => plan,
        };
        let engine = Arc::new(self.open_plan(&plan)?);
        plan.insert(&mut catalog, &engine);
        drop(catalog);
        self.queue_node_import(&plan);
        Ok(engine)
    }

    fn open_plan(&self, plan: &OpenPlan) -> ApiResult<Engine> {
        match plan {
            OpenPlan::Node(source) => self.open_node_engine(source),
            OpenPlan::Workspace {
                project,
                sources,
                is_remount,
            } => {
                let generation = self.0.next_generation.fetch_add(1, Ordering::Relaxed);
                let engine = Engine::open_workspace(project, sources, &self.0.dirs, generation)
                    .map_err(invalid_value)?;
                if *is_remount {
                    for source in sources {
                        engine.mount(project, source).map_err(invalid_value)?;
                    }
                }
                Ok(engine)
            }
        }
    }

    fn queue_node_import(&self, plan: &OpenPlan) {
        if let OpenPlan::Node(source) = plan {
            self.queue_import(source);
        }
    }

    /// Mounts `sources` into the project's workspace under the engine lock only.
    /// A failed mount may leave half-made schemas, so the workspace is dropped.
    pub(crate) fn mount_sources(
        &self,
        project_id: &str,
        sources: &[SourceRecord],
    ) -> ApiResult<Vec<ViewInfo>> {
        let project = self.catalog().project(project_id)?.clone();
        let engine = self.engine_for_node(&project.node_id)?;
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

/// The engine for `node_id` if one is open, else what opening it needs.
fn snapshot(catalog: &Catalog, node_id: &str) -> ApiResult<Snapshot> {
    if let Some(engine) = catalog.engines.get(node_id) {
        return Ok(Snapshot::Ready(Arc::clone(engine)));
    }
    if let Some(project) = catalog
        .projects
        .iter()
        .find(|project| project.node_id == node_id)
    {
        return Ok(Snapshot::Open(OpenPlan::Workspace {
            project: project.clone(),
            sources: catalog.sources_of(&project.id).cloned().collect(),
            is_remount: catalog.remount.contains(node_id),
        }));
    }
    catalog
        .sources_of(DEFAULT_PROJECT_ID)
        .find(|source| source.id == node_id)
        .map(|source| Snapshot::Open(OpenPlan::Node(source.clone())))
        .ok_or_else(|| ApiError::not_found("Node not found"))
}

#[cfg(test)]
thread_local! {
    static BEFORE_OPEN: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Runs this thread's test hook, which pauses an open after its snapshot.
#[cfg(test)]
fn before_open() {
    let hook = BEFORE_OPEN.with(|slot| slot.borrow().clone());
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(not(test))]
fn before_open() {}

/// Makes this thread's engine opens call `hook` between their snapshot and the open.
#[cfg(test)]
fn set_before_open(hook: impl Fn() + 'static) {
    BEFORE_OPEN.with(|slot| *slot.borrow_mut() = Some(Rc::new(hook)));
}

#[cfg(test)]
mod tests;
