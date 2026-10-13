//! Tests for registering uploads and deleting sources.

use crate::runtime::catalog::SourceSummary;
use crate::runtime::registry::{RawRecord, load_registry};
use crate::runtime::state::fixtures::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

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
