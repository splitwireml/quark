//! Tests for listing, creating and isolating projects.

use crate::runtime::registry::load_projects;
use crate::runtime::state::fixtures::*;
use serde_json::json;

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
