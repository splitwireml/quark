//! Builders and waiters the runtime tests share.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::error::ApiResult;
use crate::runtime::dirs::Dirs;
use crate::runtime::mount::ViewInfo;
use crate::runtime::state::AppState;

pub(crate) fn dirs(root: &Path) -> Dirs {
    Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    }
}

pub(crate) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub(crate) fn entry(root: &Path, id: &str, project: Option<&str>) -> Value {
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

pub(crate) fn project(id: &str) -> Value {
    json!({"id": id, "name": format!("Project {id}"), "node_id": format!("project_{id}")})
}

/// Loads state with the columnar worker running, so mounted sources get imported.
pub(crate) fn started_with_worker(root: &Path, registry: &[Value], projects: &[Value]) -> AppState {
    let dirs = dirs(root);
    write(&dirs.registry_file(), &json!(registry).to_string());
    write(&dirs.projects_file(), &json!(projects).to_string());
    AppState::load(dirs).unwrap()
}

/// Loads state with the worker already stopped, so a finished import cannot drop an engine
/// while a test counts them.
pub(crate) fn started(root: &Path, registry: &[Value], projects: &[Value]) -> AppState {
    let state = started_with_worker(root, registry, projects);
    state.shutdown();
    state
}

pub(crate) fn failure<T>(result: ApiResult<T>) -> (u16, String) {
    match result {
        Err(error) => (error.status().as_u16(), error.detail().to_owned()),
        Ok(_) => panic!("expected an error"),
    }
}

pub(crate) const WAIT: Duration = Duration::from_secs(60);

pub(crate) fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn cache_files(dirs: &Dirs) -> Vec<PathBuf> {
    fs::read_dir(dirs.columnar())
        .unwrap()
        .flatten()
        .map(|item| item.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "duckdb"))
        .collect()
}

/// How many of the project's source views are defined over an attached cache.
pub(crate) fn cached_view_count(state: &AppState) -> usize {
    let engine = state.engine_for_node("project_p").unwrap();
    let inner = engine.lock();
    let count: i64 = inner
        .conn
        .query_row(
            "SELECT count(*) FROM duckdb_views()
             WHERE schema_name LIKE 'source_%' AND contains(sql, 'cache_')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    usize::try_from(count).unwrap()
}

/// Mounts the sources of project `p` again and again until `caches` cache files exist and
/// every view reads from one. Waiting for the engines to empty instead misses a swap that
/// happened before the first mount.
pub(crate) fn mount_and_wait_for_cache(state: &AppState, caches: usize) -> Vec<ViewInfo> {
    let mut views = Vec::new();
    wait_until("the import and the engine swap", || {
        views = state.project_views("p").unwrap();
        cache_files(&state.0.dirs).len() == caches && cached_view_count(state) == views.len()
    });
    views
}
