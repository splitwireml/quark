//! Shared app state: where the data lives and which sources are in the catalog.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::Context;
use serde_json::Value;

use crate::engine::{Dirs, Engine};
use crate::registry::{
    ProjectRecord, RawRecord, SourceRecord, default_project, load_projects, load_registry,
};

pub struct Catalog {
    /// The default project first, then the saved ones.
    pub projects: Vec<ProjectRecord>,
    /// Every stored entry, including inactive ones and unknown keys.
    pub registry: Vec<RawRecord>,
    /// The entries whose file exists, in registry order.
    pub sources: Vec<SourceRecord>,
    /// Opened engines by node id; nothing is opened at startup.
    pub engines: HashMap<String, Arc<Engine>>,
}

pub struct Shared {
    pub dirs: Dirs,
    pub catalog: Mutex<Catalog>,
    pub next_generation: AtomicU64,
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

        Ok(Self(Arc::new(Shared {
            dirs,
            catalog: Mutex::new(Catalog {
                projects,
                registry,
                sources,
                engines: HashMap::new(),
            }),
            next_generation: AtomicU64::new(1),
        })))
    }

    /// Locks the catalog, recovering it if a panicking thread poisoned the mutex.
    pub fn catalog(&self) -> MutexGuard<'_, Catalog> {
        self.0
            .catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Drops every engine, closing its connection.
    pub fn shutdown(&self) {
        let engines = std::mem::take(&mut self.catalog().engines);
        drop(engines);
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
}
