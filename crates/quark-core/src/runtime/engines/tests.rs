//! Tests for engine resolution, mount failures and the swap to cached views.

use crate::runtime::engine::EngineKind;
use crate::runtime::state::AppState;
use crate::runtime::state::fixtures::*;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

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

#[test]
fn views_switch_to_cache_with_identical_rows() {
    let root = tempfile::tempdir().unwrap();
    let sources = [entry(root.path(), "s", Some("p"))];
    let state = started_with_worker(root.path(), &sources, &[project("p")]);

    let live = state.project_views("p").unwrap();
    let (live_rows, _) = read_view(&state, &live[0].sql);
    let cached = mount_and_wait_for_cache(&state, 1);
    let (cached_rows, definition) = read_view(&state, &cached[0].sql);

    assert_eq!(cached, live);
    assert_eq!(live_rows.len(), 2);
    assert_eq!(cached_rows, live_rows);
    assert!(definition.contains("cache_"), "{definition}");
    state.shutdown();
}

/// Rewinds a file's mtime the way `ColumnarKey::touch` writes it, so a file DuckDB holds open
/// stays reachable on Windows.
fn set_modified_time(path: &Path, time: SystemTime) {
    let mut options = fs::File::options();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.access_mode(0x0100); // FILE_WRITE_ATTRIBUTES
    }
    #[cfg(not(windows))]
    options.write(true);
    options.open(path).unwrap().set_modified(time).unwrap();
}

#[test]
fn mounting_touches_the_cache_file() {
    let root = tempfile::tempdir().unwrap();
    let sources = [entry(root.path(), "s", Some("p"))];
    let state = started_with_worker(root.path(), &sources, &[project("p")]);
    mount_and_wait_for_cache(&state, 1);
    let file = cache_files(&state.0.dirs).remove(0);
    let day = Duration::from_secs(86_400);
    let old = SystemTime::now() - 3 * day;
    set_modified_time(&file, old);

    state.project_views("p").unwrap();

    assert!(fs::metadata(&file).unwrap().modified().unwrap() > old + day);
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
    let views = mount_and_wait_for_cache(&second, 2);
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
