//! Shared app state: where the data lives and which sources are in the catalog.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cache::columnar::ColumnarWorker;
use crate::engine::{DatasetInfo, Dirs, Engine, describe};
use crate::error::{ApiError, ApiResult};
use crate::mount::{ViewInfo, import_job};
use crate::naming::dataset_name;
use crate::registry::{
    DEFAULT_PROJECT_ID, ProjectRecord, RawRecord, SourceRecord, default_project, load_projects,
    load_registry, save_projects, save_registry,
};
use crate::sql::quote_ident;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetEntry {
    #[serde(flatten)]
    pub info: DatasetInfo,
    pub columns: Vec<String>,
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
    remount: HashSet<String>,
}

impl Catalog {
    fn project(&self, id: &str) -> ApiResult<&ProjectRecord> {
        self.projects
            .iter()
            .find(|project| project.id == id)
            .ok_or_else(|| ApiError::not_found("Project not found"))
    }

    /// The project's active sources in registry order.
    fn sources_of<'a>(&'a self, project_id: &'a str) -> impl Iterator<Item = &'a SourceRecord> {
        self.sources
            .iter()
            .filter(move |source| source.project() == project_id)
    }

    fn summary(&self, project: &ProjectRecord) -> ProjectSummary {
        ProjectSummary {
            project: project.clone(),
            source_count: self.sources_of(&project.id).count(),
        }
    }
}

fn cannot_open(error: impl Display) -> ApiError {
    ApiError::bad_request(format!("Could not open source: {error}"))
}

fn registry_not_saved(error: io::Error) -> ApiError {
    tracing::error!(%error, "could not save registry");
    ApiError::internal("Internal error")
}

/// Python's global DuckDB error handler answers every engine failure with this 422.
fn invalid_value(error: impl Display) -> ApiError {
    ApiError::unprocessable(format!("Invalid filter value: {error}"))
}

pub struct Shared {
    pub dirs: Dirs,
    pub catalog: Mutex<Catalog>,
    pub next_generation: AtomicU64,
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
            let on_ready = Box::new(move |source_id: String| {
                if let Some(shared) = weak.upgrade() {
                    Self(shared).drop_engines_of(&source_id);
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
                next_generation: AtomicU64::new(1),
                columnar: Mutex::new(Some(worker)),
            }
        });
        Ok(Self(shared))
    }

    /// Queues the source's columnar import unless its cache is already there.
    /// Repeats are cheap: the worker skips finished and failed imports.
    fn queue_import(&self, source: &SourceRecord) {
        let Some(job) = import_job(source) else {
            return;
        };
        if job.key.is_ready(&self.0.dirs.cache) {
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
    /// Work already running finishes on the old engine.
    fn drop_engines_of(&self, source_id: &str) {
        let engines = {
            let mut catalog = self.catalog();
            let Some(source) = catalog.sources.iter().find(|source| source.id == source_id) else {
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
        crate::engine::datasets(&inner.conn)
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
    fn mount_sources(
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
            remove_upload(path);
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
        crate::engine::datasets(&engine.lock().conn).map_err(cannot_open)?;

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
        let (source, engines) = {
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
            (source, engines)
        };
        drop(engines);
        if source.kind == "upload" {
            remove_upload(&source.source);
        }
        let mut catalog = self.catalog();
        catalog
            .registry
            .retain(|record| record.get("id").and_then(Value::as_str) != Some(node_id));
        save_registry(&self.0.dirs.registry_file(), &catalog.registry).map_err(registry_not_saved)
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

/// Deletes an upload, retrying once after 100 ms because Windows may still hold the file.
/// A file that stays behind is an orphan, which the next startup deletes.
fn remove_upload(path: &Path) {
    let remove = || match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    };
    if remove().is_err() {
        thread::sleep(Duration::from_millis(100));
        if let Err(error) = remove() {
            tracing::warn!(path = %path.display(), %error, "could not delete upload; leaving it for startup cleanup");
        }
    }
}

/// Deletes files in `uploads` that no registry entry points at, comparing canonical paths.
fn remove_orphan_uploads(uploads: &Path, registry: &[RawRecord]) -> anyhow::Result<()> {
    let referenced: Vec<PathBuf> = registry
        .iter()
        .filter_map(|record| record.get("source").and_then(Value::as_str))
        .map(|source| fs::canonicalize(source).unwrap_or_else(|_| PathBuf::from(source)))
        .collect();
    for entry in fs::read_dir(uploads).context("listing uploads")? {
        let path = entry.context("listing uploads")?.path();
        let Ok(canonical) = fs::canonicalize(&path) else {
            continue;
        };
        if canonical.is_file()
            && !referenced.contains(&canonical)
            && let Err(error) = fs::remove_file(&path)
        {
            tracing::warn!(path = %path.display(), %error, "could not delete orphan upload");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineKind;
    use serde_json::json;
    use std::time::Instant;

    fn dirs(root: &Path) -> Dirs {
        Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        }
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn startup_never_rewrites_registry() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        let kept = dirs.uploads().join("a.csv");
        write(&kept, "x\n1\n");
        let registry = format!(
            r#"[{{"id":"a","name":"A","kind":"csv","source":{:?},"future":{{"x":[1]}}}},"junk",{{"id":"gone"}}]"#,
            kept.to_string_lossy()
        );
        let projects = r#"[{"id":"p","name":"P","node_id":"project_p"},{"id":"bad"}]"#;
        write(&dirs.registry_file(), &registry);
        write(&dirs.projects_file(), projects);

        let state = AppState::load(dirs.clone()).unwrap();

        assert_eq!(fs::read_to_string(dirs.registry_file()).unwrap(), registry);
        assert_eq!(fs::read_to_string(dirs.projects_file()).unwrap(), projects);
        let catalog = state.catalog();
        assert_eq!(catalog.projects.len(), 2);
        assert_eq!(catalog.projects[0], default_project());
        assert_eq!(catalog.projects[1].id, "p");
        assert_eq!(catalog.registry.len(), 2);
        assert!(catalog.engines.is_empty());
    }

    #[test]
    fn startup_with_empty_data_folder_writes_no_json() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        AppState::load(dirs.clone()).unwrap();
        assert!(!dirs.registry_file().exists());
        assert!(!dirs.projects_file().exists());
        assert!(dirs.uploads().is_dir() && dirs.columnar().is_dir() && dirs.spill().is_dir());
    }

    #[test]
    fn missing_files_are_inactive_but_kept() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        let present = root.path().join("present.csv");
        write(&present, "x\n1\n");
        let missing = root.path().join("missing.csv");
        let record = |id: &str, path: &Path| {
            format!(
                r#"{{"id":"{id}","name":"{id}","kind":"csv","source":{:?}}}"#,
                path.to_string_lossy()
            )
        };
        write(
            &dirs.registry_file(),
            &format!(
                "[{},{}]",
                record("gone", &missing),
                record("here", &present)
            ),
        );

        let state = AppState::load(dirs).unwrap();

        let catalog = state.catalog();
        let active: Vec<_> = catalog.sources.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(active, ["here"]);
        assert_eq!(catalog.registry.len(), 2);
        assert!(catalog.engines.is_empty());
    }

    #[test]
    fn orphan_uploads_are_deleted() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        let kept = dirs.uploads().join("kept.csv");
        let orphan = dirs.uploads().join("orphan.csv");
        let nested = dirs.uploads().join("nested").join("file.csv");
        for path in [&kept, &orphan, &nested] {
            write(path, "x\n1\n");
        }
        let roundabout = dirs.uploads().join("nested").join("..").join("kept.csv");
        write(
            &dirs.registry_file(),
            &format!(
                r#"[{{"id":"a","name":"A","kind":"csv","source":{:?}}}]"#,
                roundabout.to_string_lossy()
            ),
        );

        AppState::load(dirs).unwrap();

        assert!(kept.exists());
        assert!(!orphan.exists());
        assert!(nested.exists());
    }

    #[test]
    fn spill_folder_is_emptied() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        write(&dirs.spill().join("old.tmp"), "spill");
        write(&dirs.spill().join("deep").join("older.tmp"), "spill");
        let columnar = dirs.columnar().join("keep.parquet");
        write(&columnar, "cache");

        AppState::load(dirs.clone()).unwrap();

        assert_eq!(fs::read_dir(dirs.spill()).unwrap().count(), 0);
        assert!(columnar.exists());
    }

    fn entry(root: &Path, id: &str, project: Option<&str>) -> Value {
        let file = root.join(format!("{id}.csv"));
        fs::write(&file, "a,b\n1,x\n2,y\n").unwrap();
        let mut value = json!({
            "id": id, "name": id.to_uppercase(), "kind": "csv", "source": file.to_string_lossy(),
        });
        if let Some(project) = project {
            value["project_id"] = json!(project);
        }
        value
    }

    fn project(id: &str) -> Value {
        json!({"id": id, "name": format!("Project {id}"), "node_id": format!("project_{id}")})
    }

    /// Loads state with the columnar worker running, so mounted sources get imported.
    fn started_with_worker(root: &Path, registry: &[Value], projects: &[Value]) -> AppState {
        let dirs = dirs(root);
        write(&dirs.registry_file(), &json!(registry).to_string());
        write(&dirs.projects_file(), &json!(projects).to_string());
        AppState::load(dirs).unwrap()
    }

    /// Loads state with the worker already stopped, so a finished import cannot drop an engine
    /// while a test counts them.
    fn started(root: &Path, registry: &[Value], projects: &[Value]) -> AppState {
        let state = started_with_worker(root, registry, projects);
        state.shutdown();
        state
    }

    fn failure<T>(result: ApiResult<T>) -> (u16, String) {
        match result {
            Err(error) => (error.status().as_u16(), error.detail().to_owned()),
            Ok(_) => panic!("expected an error"),
        }
    }

    #[test]
    fn projects_list_default_first_with_counts() {
        let root = tempfile::tempdir().unwrap();
        let mut gone = entry(root.path(), "gone", Some("p"));
        gone["source"] = json!(root.path().join("missing.csv").to_string_lossy());
        let state = started(
            root.path(),
            &[
                entry(root.path(), "legacy", None),
                entry(root.path(), "one", Some("p")),
                entry(root.path(), "two", Some("p")),
                gone,
            ],
            &[project("p")],
        );

        let listed = serde_json::to_value(state.projects()).unwrap();

        assert_eq!(
            listed,
            json!([
                {"id": "default", "name": "Default", "node_id": "project_default", "source_count": 1},
                {"id": "p", "name": "Project p", "node_id": "project_p", "source_count": 2},
            ])
        );
    }

    #[test]
    fn create_project_validation() {
        let root = tempfile::tempdir().unwrap();
        let state = started(root.path(), &[], &[]);
        let required = (422, "Project name is required".to_owned());
        for bad in ["", "   \t", &"x".repeat(201)] {
            assert_eq!(failure(state.create_project(bad)), required, "{bad:?}");
        }
        assert!(load_projects(&state.0.dirs.projects_file()).is_empty());

        let created = state.create_project("  Quarterly  ").unwrap();
        assert_eq!(created.project.name, "Quarterly");
        assert_eq!(created.source_count, 0);
        assert_eq!(created.project.id.len(), 32);
        assert!(
            created
                .project
                .id
                .chars()
                .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
        );
        assert_eq!(
            created.project.node_id,
            format!("project_{}", created.project.id)
        );
        state.create_project(&"\u{1F600}".repeat(200)).unwrap();

        let saved = load_projects(&state.0.dirs.projects_file());
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0], created.project);
        assert!(saved.iter().all(|saved| saved.id != "default"));
        assert_eq!(state.projects().len(), 3);
    }

    #[test]
    fn project_isolation() {
        let root = tempfile::tempdir().unwrap();
        let state = started(
            root.path(),
            &[
                entry(root.path(), "legacy", None),
                entry(root.path(), "a1", Some("a")),
                entry(root.path(), "b1", Some("b")),
                entry(root.path(), "b2", Some("b")),
            ],
            &[project("a"), project("b")],
        );

        let names = |project: &str| -> Vec<String> {
            let sources = state.project_sources(project).unwrap();
            sources.into_iter().map(|source| source.id).collect()
        };
        assert_eq!(names("default"), ["legacy"]);
        assert_eq!(names("a"), ["a1"]);
        assert_eq!(names("b"), ["b1", "b2"]);
        let legacy: Vec<_> = state.legacy_nodes().into_iter().map(|n| n.id).collect();
        assert_eq!(legacy, ["legacy"]);

        let detail = state.source_detail("b", "b2").unwrap();
        assert_eq!((detail.name.as_str(), detail.kind.as_str()), ("B2", "csv"));
        assert_eq!(detail.project_id, "b");
        assert_eq!(detail.views.len(), 1);
        assert_eq!(detail.views[0].source_id, "b2");
        assert_eq!(
            failure(state.source_detail("a", "b2")),
            (404, "Source not found".to_owned())
        );
        for unknown in [
            failure(state.project_sources("nope")),
            failure(state.source_detail("nope", "a1")),
            failure(state.project_views("nope")),
        ] {
            assert_eq!(unknown, (404, "Project not found".to_owned()));
        }

        let views = state.project_views("b").unwrap();
        let sources: Vec<_> = views.iter().map(|view| view.source_id.as_str()).collect();
        assert_eq!(sources, ["b1", "b2"]);
        assert!(views.iter().all(|view| view.project_id == "b"));
        let datasets = state.datasets("project_b").unwrap();
        assert_eq!(datasets.len(), 2);
        assert!(state.datasets("project_a").unwrap().is_empty());
    }

    #[test]
    fn mount_failure_drops_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let broken = root.path().join("broken.duckdb");
        fs::write(&broken, "not a database").unwrap();
        let mut bad = entry(root.path(), "bad", Some("p"));
        bad["source"] = json!(broken.to_string_lossy());
        let state = started(
            root.path(),
            &[entry(root.path(), "good", Some("p")), bad],
            &[project("p")],
        );

        let (status, detail) = failure(state.project_views("p"));

        assert_eq!(status, 422);
        assert!(detail.starts_with("Invalid filter value: "), "{detail}");
        assert!(!state.catalog().engines.contains_key("project_p"));
    }

    #[test]
    fn engine_resolution() {
        let root = tempfile::tempdir().unwrap();
        let mut unreadable = entry(root.path(), "text", None);
        let notes = root.path().join("notes.txt");
        fs::write(&notes, "hello").unwrap();
        unreadable["source"] = json!(notes.to_string_lossy());
        let state = started(
            root.path(),
            &[
                entry(root.path(), "legacy", None),
                entry(root.path(), "a1", Some("a")),
                unreadable,
            ],
            &[project("a")],
        );

        let default = state.engine_for_node("project_default").unwrap();
        assert_eq!(
            default.kind,
            EngineKind::Workspace {
                project_id: "default".to_owned(),
                node_id: "project_default".to_owned()
            }
        );
        assert!(Arc::ptr_eq(
            &default,
            &state.engine_for_node("project_default").unwrap()
        ));
        let workspace = state.engine_for_node("project_a").unwrap();
        assert!(!Arc::ptr_eq(&default, &workspace));

        let node = state.engine_for_node("legacy").unwrap();
        assert_eq!(
            node.kind,
            EngineKind::Node {
                node_id: "legacy".to_owned()
            }
        );
        assert!(Arc::ptr_eq(
            &node,
            &state.engine_for_node("legacy").unwrap()
        ));
        assert_eq!(
            serde_json::to_value(state.datasets("legacy").unwrap()).unwrap(),
            json!([{
                "id": "WyJtYWluIiwiZGF0YSJd", "name": "data", "schema": "main",
                "type": "VIEW", "columns": ["a", "b"],
            }])
        );

        for missing in ["a1", "nope", "gone"] {
            assert_eq!(
                failure(state.engine_for_node(missing)),
                (404, "Node not found".to_owned()),
                "{missing}"
            );
        }
        let (status, detail) = failure(state.engine_for_node("text"));
        assert_eq!(status, 400);
        assert!(detail.starts_with("Could not open source: "), "{detail}");
    }

    fn ids(sources: Vec<SourceSummary>) -> Vec<String> {
        sources.into_iter().map(|source| source.id).collect()
    }

    fn keys(record: &RawRecord) -> Vec<&str> {
        record.keys().map(String::as_str).collect()
    }

    fn make_database(path: &Path) {
        duckdb::Connection::open(path)
            .unwrap()
            .execute_batch("CREATE TABLE items (a INTEGER)")
            .unwrap();
    }

    #[test]
    fn upload_registers_and_lists() {
        let root = tempfile::tempdir().unwrap();
        let state = started(root.path(), &[], &[project("p")]);
        assert_eq!(
            state.upload_path("abc", ".csv"),
            state.0.dirs.uploads().join("abc.csv")
        );

        let path = state.upload_path("n1", ".csv");
        fs::write(&path, "a,b\n1,x\n").unwrap();
        state.project_views("default").unwrap();
        assert!(state.catalog().engines.contains_key("project_default"));
        let public = state
            .register_upload(None, "n1", "reports/Sales Data.csv", &path)
            .unwrap();
        assert_eq!(
            public,
            json!({"id": "n1", "name": "Sales Data.csv", "kind": "upload", "source": path.to_string_lossy()})
        );
        assert_eq!(
            keys(public.as_object().unwrap()),
            ["id", "name", "kind", "source"]
        );
        {
            let catalog = state.catalog();
            assert!(catalog.engines.contains_key("n1"));
            assert!(!catalog.engines.contains_key("project_default"));
        }
        assert_eq!(ids(state.project_sources("default").unwrap()), ["n1"]);
        assert_eq!(state.legacy_nodes()[0].id, "n1");
        let views = state.project_views("default").unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].source_id, "n1");

        let path = state.upload_path("n2", ".csv");
        fs::write(&path, "a,b\n1,x\n").unwrap();
        let public = state
            .register_upload(Some("p"), "n2", "Other.csv", &path)
            .unwrap();
        assert_eq!(
            public,
            json!({"id": "n2", "name": "Other.csv", "kind": "upload", "project_id": "p"})
        );
        assert_eq!(
            keys(public.as_object().unwrap()),
            ["id", "name", "kind", "project_id"]
        );
        assert!(!state.catalog().engines.contains_key("n2"));
        assert_eq!(ids(state.project_sources("p").unwrap()), ["n2"]);
        assert_eq!(ids(state.project_sources("default").unwrap()), ["n1"]);

        let path = state.upload_path("n3", ".DB");
        make_database(&path);
        state
            .register_upload(Some("p"), "n3", "Facts.DB", &path)
            .unwrap();

        let saved = load_registry(&state.0.dirs.registry_file());
        let key_lists: Vec<_> = saved.iter().map(keys).collect();
        assert_eq!(
            key_lists,
            [
                vec!["id", "name", "kind", "source", "dataset_name"],
                vec!["id", "name", "kind", "source", "project_id", "dataset_name"],
                vec!["id", "name", "kind", "source", "project_id"],
            ]
        );
        assert_eq!(saved[0]["dataset_name"], "sales_data");

        let path = state.upload_path("n4", ".csv");
        fs::write(&path, "a\n1\n").unwrap();
        assert_eq!(
            failure(state.register_upload(Some("nope"), "n4", "x.csv", &path)),
            (404, "Project not found".to_owned())
        );
        assert!(!path.exists());
        assert_eq!(state.catalog().registry.len(), 3);
    }

    #[test]
    fn unreadable_upload_is_rejected_and_removed() {
        let root = tempfile::tempdir().unwrap();
        let state = started(root.path(), &[], &[]);
        for (ext, bytes) in [(".duckdb", "not a database"), (".txt", "hello")] {
            let path = state.upload_path("bad", ext);
            fs::write(&path, bytes).unwrap();

            let (status, detail) =
                failure(state.register_upload(None, "bad", &format!("bad{ext}"), &path));

            assert_eq!(status, 400, "{ext}");
            assert!(detail.starts_with("Could not open source: "), "{detail}");
            assert!(!path.exists(), "{ext}");
        }
        let catalog = state.catalog();
        assert!(catalog.registry.is_empty() && catalog.sources.is_empty());
        assert!(catalog.engines.is_empty());
        assert!(load_registry(&state.0.dirs.registry_file()).is_empty());
    }

    #[test]
    fn delete_releases_and_removes_file() {
        let root = tempfile::tempdir().unwrap();
        let state = started(root.path(), &[], &[project("p")]);
        let database = state.upload_path("db", ".duckdb");
        make_database(&database);
        state
            .register_upload(None, "db", "facts.duckdb", &database)
            .unwrap();
        let sheet = state.upload_path("csv", ".csv");
        fs::write(&sheet, "a\n1\n").unwrap();
        state
            .register_upload(Some("p"), "csv", "sheet.csv", &sheet)
            .unwrap();
        state.project_views("default").unwrap();
        state.project_views("p").unwrap();
        state.engine_for_node("db").unwrap();
        assert_eq!(state.catalog().engines.len(), 3);

        state.delete_source("default", "db").unwrap();

        assert!(!database.exists());
        {
            let catalog = state.catalog();
            let open: Vec<_> = catalog.engines.keys().map(String::as_str).collect();
            assert_eq!(open, ["project_p"]);
            assert_eq!(catalog.registry.len(), 1);
        }
        state.delete_source("p", "csv").unwrap();
        assert!(!sheet.exists());
        assert!(state.catalog().engines.is_empty());
        assert!(load_registry(&state.0.dirs.registry_file()).is_empty());
    }

    #[test]
    fn delete_rules() {
        let root = tempfile::tempdir().unwrap();
        let mut gone = entry(root.path(), "gone", Some("a"));
        gone["source"] = json!(root.path().join("missing.csv").to_string_lossy());
        let mut kept = entry(root.path(), "b1", Some("b"));
        kept["future"] = json!({"x": [1]});
        let legacy = entry(root.path(), "legacy", None);
        let attached = entry(root.path(), "a1", Some("a"));
        let attached_file = PathBuf::from(attached["source"].as_str().unwrap());
        let state = started(
            root.path(),
            &[legacy, attached, kept, gone],
            &[project("a"), project("b")],
        );

        let node_missing = (404, "Node not found".to_owned());
        assert_eq!(
            failure(state.delete_source("nope", "a1")),
            (404, "Project not found".to_owned())
        );
        for (project, node) in [
            ("a", "nope"),
            ("b", "a1"),
            ("default", "a1"),
            ("a", "gone"),
            ("a", "legacy"),
        ] {
            assert_eq!(
                failure(state.delete_source(project, node)),
                node_missing,
                "{project}/{node}"
            );
        }
        assert_eq!(state.catalog().registry.len(), 4);

        state.delete_source("a", "a1").unwrap();

        assert!(attached_file.exists(), "only uploads lose their file");
        assert_eq!(failure(state.delete_source("a", "a1")), node_missing);
        state.delete_source("default", "legacy").unwrap();
        let saved = load_registry(&state.0.dirs.registry_file());
        let left: Vec<_> = saved
            .iter()
            .map(|record| record["id"].as_str().unwrap())
            .collect();
        assert_eq!(left, ["b1", "gone"]);
        assert_eq!(saved[0]["future"], json!({"x": [1]}));
        assert_eq!(
            ids(state.project_sources("a").unwrap()),
            Vec::<String>::new()
        );
        assert_eq!(ids(state.project_sources("b").unwrap()), ["b1"]);
    }

    const WAIT: Duration = Duration::from_secs(60);

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn cache_files(dirs: &Dirs) -> Vec<PathBuf> {
        fs::read_dir(dirs.columnar())
            .unwrap()
            .flatten()
            .map(|item| item.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "duckdb"))
            .collect()
    }

    /// The rows of the project's mounted view as text, and how that view is defined.
    fn read_view(state: &AppState, view_sql: &str) -> (Vec<String>, String) {
        let engine = state.engine_for_node("project_p").unwrap();
        let inner = engine.lock();
        let rows = inner
            .conn
            .prepare(&format!(
                "SELECT CAST(t AS VARCHAR) FROM ({view_sql}) t ORDER BY 1"
            ))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let definition = inner
            .conn
            .query_row(
                "SELECT sql FROM duckdb_views() WHERE schema_name LIKE 'source_%' AND view_name = 'data'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        (rows, definition)
    }

    /// Mounts the one source of project `p` and waits until its import has replaced the engine.
    fn mount_and_wait_for_cache(state: &AppState, caches: usize) -> Vec<ViewInfo> {
        let views = state.project_views("p").unwrap();
        wait_until("the import", || cache_files(&state.0.dirs).len() == caches);
        wait_until("the engine swap", || state.catalog().engines.is_empty());
        views
    }

    #[test]
    fn views_switch_to_cache_with_identical_rows() {
        let root = tempfile::tempdir().unwrap();
        let sources = [entry(root.path(), "s", Some("p"))];
        let state = started_with_worker(root.path(), &sources, &[project("p")]);

        let live = state.project_views("p").unwrap();
        let (live_rows, _) = read_view(&state, &live[0].sql);
        wait_until("the engine swap", || state.catalog().engines.is_empty());
        let cached = state.project_views("p").unwrap();
        let (cached_rows, definition) = read_view(&state, &cached[0].sql);

        assert_eq!(cached, live);
        assert_eq!(live_rows.len(), 2);
        assert_eq!(cached_rows, live_rows);
        assert!(definition.contains("cache_"), "{definition}");
        state.shutdown();
    }

    #[test]
    fn restart_mounts_cache_directly() {
        let root = tempfile::tempdir().unwrap();
        let sources = [entry(root.path(), "s", Some("p"))];
        let first = started_with_worker(root.path(), &sources, &[project("p")]);
        mount_and_wait_for_cache(&first, 1);
        first.shutdown();

        let second = AppState::load(dirs(root.path())).unwrap();
        let views = second.project_views("p").unwrap();
        let (rows, definition) = read_view(&second, &views[0].sql);

        assert_eq!(rows.len(), 2);
        assert!(definition.contains("cache_"), "{definition}");
        assert_eq!(cache_files(&second.0.dirs).len(), 1);
        assert_eq!(second.catalog().engines.len(), 1);
        second.shutdown();
    }

    #[test]
    fn edited_source_gets_a_new_cache() {
        let root = tempfile::tempdir().unwrap();
        let sources = [entry(root.path(), "s", Some("p"))];
        let first = started_with_worker(root.path(), &sources, &[project("p")]);
        mount_and_wait_for_cache(&first, 1);
        let views = first.project_views("p").unwrap();
        let (_, old_definition) = read_view(&first, &views[0].sql);
        first.shutdown();
        write(&root.path().join("s.csv"), "a,b\n1,x\n2,y\n3,z\n");

        let second = AppState::load(dirs(root.path())).unwrap();
        let views = second.project_views("p").unwrap();
        let (rows, _) = read_view(&second, &views[0].sql);
        assert_eq!(rows.len(), 3);
        mount_and_wait_for_cache(&second, 2);
        let views = second.project_views("p").unwrap();
        let (rows, new_definition) = read_view(&second, &views[0].sql);

        assert_eq!(rows.len(), 3);
        assert!(new_definition.contains("cache_"), "{new_definition}");
        assert_ne!(new_definition, old_definition);
        second.shutdown();
    }

    #[test]
    fn purged_cache_falls_back_to_live() {
        let root = tempfile::tempdir().unwrap();
        let sources = [entry(root.path(), "s", Some("p"))];
        let first = started_with_worker(root.path(), &sources, &[project("p")]);
        mount_and_wait_for_cache(&first, 1);
        first.shutdown();
        for file in cache_files(&first.0.dirs) {
            fs::remove_file(file).unwrap();
        }

        let second = AppState::load(dirs(root.path())).unwrap();
        let views = second.project_views("p").unwrap();
        let (rows, _) = read_view(&second, &views[0].sql);

        assert_eq!(rows.len(), 2);
        wait_until("a new import", || cache_files(&second.0.dirs).len() == 1);
        second.shutdown();
    }
}
