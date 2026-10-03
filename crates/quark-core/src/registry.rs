//! The projects file: `projects.json` in the data folder.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_PROJECT_ID: &str = "default";
pub const DEFAULT_PROJECT_NAME: &str = "Default";
pub const DEFAULT_PROJECT_NODE_ID: &str = "project_default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub node_id: String,
}

pub fn default_project() -> ProjectRecord {
    ProjectRecord {
        id: DEFAULT_PROJECT_ID.to_owned(),
        name: DEFAULT_PROJECT_NAME.to_owned(),
        node_id: DEFAULT_PROJECT_NODE_ID.to_owned(),
    }
}

/// Reads the saved projects, never including the default one. A missing,
/// malformed or non-array file gives an empty list.
pub fn load_projects(path: &Path) -> Vec<ProjectRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut seen = HashSet::from([DEFAULT_PROJECT_ID.to_owned()]);
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<ProjectRecord>(item).ok())
        .filter(|project| seen.insert(project.id.clone()))
        .collect()
}

/// Writes the projects atomically: pretty JSON to `<file>.tmp`, then a rename.
pub fn save_projects(path: &Path, projects: &[ProjectRecord]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, serde_json::to_string_pretty(projects)?)?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
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
}
