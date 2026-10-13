//! Tests for the projects and registry files.

use super::*;

fn record(id: &str, name: &str) -> ProjectRecord {
    ProjectRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        node_id: format!("project_{id}"),
    }
}

#[test]
fn missing_or_malformed_projects_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projects.json");
    assert!(load_projects(&path).is_empty());
    for text in ["not json", "{\"id\": \"a\"}", "\"text\"", ""] {
        std::fs::write(&path, text).unwrap();
        assert!(load_projects(&path).is_empty(), "{text:?}");
    }
}

#[test]
fn invalid_and_duplicate_entries_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projects.json");
    std::fs::write(
        &path,
        r#"[
            {"id": "a", "name": "First", "node_id": "project_a"},
            {"id": "a", "name": "Again", "node_id": "project_a2"},
            {"id": "default", "name": "Default", "node_id": "project_default"},
            {"id": "b", "name": "No node"},
            {"id": 7, "name": "Numeric id", "node_id": "project_7"},
            "text",
            {"id": "c", "name": "Third", "node_id": "project_c"}
        ]"#,
    )
    .unwrap();
    assert_eq!(
        load_projects(&path),
        vec![record("a", "First"), record("c", "Third")]
    );
}

#[test]
fn save_is_atomic_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("projects.json");
    let projects = vec![record("a", "First"), record("b", "Second")];
    save_projects(&path, &projects).unwrap();
    assert_eq!(load_projects(&path), projects);
    assert!(!path.with_extension("json.tmp").exists());
    assert!(std::fs::read_to_string(&path).unwrap().contains("\n  "));
}

fn raw(text: &str) -> RawRecord {
    match serde_json::from_str::<Value>(text).unwrap() {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

#[test]
fn non_object_entries_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");
    assert!(load_registry(&path).is_empty());
    std::fs::write(&path, r#"[{"id": "a"}, "text", 7, null, [1], {"id": "b"}]"#).unwrap();
    assert_eq!(
        load_registry(&path),
        vec![raw(r#"{"id": "a"}"#), raw(r#"{"id": "b"}"#)]
    );
    for text in ["not json", "{\"id\": \"a\"}", ""] {
        std::fs::write(&path, text).unwrap();
        assert!(load_registry(&path).is_empty(), "{text:?}");
    }
}

#[test]
fn unknown_keys_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("registry.json");
    let records = vec![
        raw(
            r#"{"id": "a", "name": "A", "kind": "csv", "source": "/a.csv", "future": {"x": [1, 2]}}"#,
        ),
        raw(r#"{"id": "b", "extra": null}"#),
    ];
    save_registry(&path, &records).unwrap();
    assert_eq!(load_registry(&path), records);
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn source_record_requires_core_fields() {
    let full = r#"{"id": "a", "name": "A", "kind": "csv", "source": "/a.csv"}"#;
    assert!(SourceRecord::from_raw(&raw(full)).is_some());
    for key in ["id", "name", "kind", "source"] {
        let mut record = raw(full);
        record.remove(key);
        assert!(SourceRecord::from_raw(&record).is_none(), "missing {key}");
        record.insert(key.to_owned(), Value::from(7));
        assert!(SourceRecord::from_raw(&record).is_none(), "numeric {key}");
    }
}

#[test]
fn source_record_reads_optional_fields() {
    let record = SourceRecord::from_raw(&raw(
        r#"{"id": "a", "name": "A", "kind": "xlsx", "source": "/a.xlsx",
            "project_id": "p", "dataset_name": "d", "sheets": ["S1", "S2"]}"#,
    ))
    .unwrap();
    assert_eq!(record.source, PathBuf::from("/a.xlsx"));
    assert_eq!(record.project(), "p");
    assert_eq!(record.dataset_name.as_deref(), Some("d"));
    assert_eq!(record.sheets, Some(vec!["S1".to_owned(), "S2".to_owned()]));
}

#[test]
fn legacy_record_defaults() {
    let record = SourceRecord::from_raw(&raw(
        r#"{"id": "a", "name": "A", "kind": "csv", "source": "/a.csv"}"#,
    ))
    .unwrap();
    assert_eq!(record.project_id, None);
    assert_eq!(record.project(), DEFAULT_PROJECT_ID);
    assert_eq!(record.dataset_name, None);
    assert_eq!(record.sheets, None);
}
