//! The data folder's two JSON files: `projects.json` and `registry.json`.

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
    write_atomic(path, projects)
}

/// A registry entry as stored: every key, including ones this build does not know.
pub type RawRecord = serde_json::Map<String, Value>;

/// Reads the registry entries, dropping any that are not objects. A missing,
/// malformed or non-array file gives an empty list.
pub fn load_registry(path: &Path) -> Vec<RawRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(record) => Some(record),
            _ => None,
        })
        .collect()
}

/// Writes the registry atomically, keeping every key of every entry.
pub fn save_registry(path: &Path, records: &[RawRecord]) -> io::Result<()> {
    write_atomic(path, records)
}

fn write_atomic(path: &Path, value: &(impl Serialize + ?Sized)) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&temporary, path)
}

/// The fields of a registry entry that the engine reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: PathBuf,
    pub project_id: Option<String>,
    pub dataset_name: Option<String>,
    pub sheets: Option<Vec<String>>,
}

impl SourceRecord {
    /// `None` unless `id`, `name`, `kind` and `source` are all strings.
    pub fn from_raw(raw: &RawRecord) -> Option<Self> {
        let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
        let sheets = raw.get("sheets").and_then(Value::as_array).map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        });
        Some(Self {
            id: text("id")?,
            name: text("name")?,
            kind: text("kind")?,
            source: PathBuf::from(text("source")?),
            project_id: text("project_id"),
            dataset_name: text("dataset_name"),
            sheets,
        })
    }

    /// The owning project; legacy uploads have no `project_id` and belong to the default one.
    pub fn project(&self) -> &str {
        self.project_id.as_deref().unwrap_or(DEFAULT_PROJECT_ID)
    }
}

#[cfg(test)]
mod tests;
