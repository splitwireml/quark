//! Tests for deleting sources: engines released, files and caches removed, registry kept intact.

use crate::runtime::registry::load_registry;
use crate::runtime::state::fixtures::*;
use serde_json::json;
use std::path::PathBuf;

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
    let catalog = state.catalog();
    assert_eq!(catalog.sources_of("a").count(), 0);
    assert_eq!(catalog.sources_of("b").count(), 1);
}

#[test]
fn deleting_source_removes_cache_files() {
    let root = tempfile::tempdir().unwrap();
    let sources = [entry(root.path(), "s", Some("p"))];
    let state = started_with_worker(root.path(), &sources, &[project("p")]);
    mount_and_wait_for_cache(&state, 1);

    state.delete_source("p", "s").unwrap();

    assert!(cache_files(&state.0.dirs).is_empty());
    state.shutdown();
}

#[test]
fn cache_shared_with_another_source_survives_a_delete() {
    let root = tempfile::tempdir().unwrap();
    let first = entry(root.path(), "s", Some("p"));
    let mut second = first.clone();
    second["id"] = json!("t");
    let state = started_with_worker(root.path(), &[first, second], &[project("p")]);
    mount_and_wait_for_cache(&state, 1);

    state.delete_source("p", "s").unwrap();
    assert_eq!(cache_files(&state.0.dirs).len(), 1);
    state.delete_source("p", "t").unwrap();

    assert!(cache_files(&state.0.dirs).is_empty());
    state.shutdown();
}
