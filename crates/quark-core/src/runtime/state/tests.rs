//! Tests for loading state and publishing a finished cache.

use super::fixtures::*;
use super::*;
use serde_json::json;
use std::fs;
use std::path::Path;

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

/// Deletes source `s` of project `p`, then publishes its cache as an import still running would.
fn publish_cache_after_delete(state: &AppState) -> ColumnarKey {
    let key = state
        .catalog()
        .sources
        .iter()
        .find_map(import_job)
        .unwrap()
        .key;
    state.delete_source("p", "s").unwrap();
    write(&key.final_path(&state.0.dirs.cache), "");
    key
}

#[test]
fn ready_cache_for_deleted_source_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let sources = [entry(root.path(), "s", Some("p"))];
    let state = started(root.path(), &sources, &[project("p")]);
    let key = publish_cache_after_delete(&state);
    assert_eq!(cache_files(&state.0.dirs).len(), 1);

    state.cache_ready("s", &key);

    assert!(!key.is_ready(&state.0.dirs.cache));
}

#[test]
fn ready_cache_for_deleted_source_stays_while_another_source_uses_it() {
    let root = tempfile::tempdir().unwrap();
    let first = entry(root.path(), "s", Some("p"));
    let mut second = first.clone();
    second["id"] = json!("t");
    let state = started(root.path(), &[first, second], &[project("p")]);
    let key = publish_cache_after_delete(&state);

    state.cache_ready("s", &key);

    assert!(key.is_ready(&state.0.dirs.cache));
}
