//! Data and cache folders, spill configuration and DuckDB lockdown.

use std::fs;
use std::io;
use std::path::PathBuf;

use duckdb::Connection;

use crate::sql::sql_string;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("filesystem error: {0}")]
    Io(#[from] io::Error),
    #[error("DuckDB error: {0}")]
    Duckdb(#[from] duckdb::Error),
}

/// The app's two roots: durable data and disposable cache.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub data: PathBuf,
    pub cache: PathBuf,
}

impl Dirs {
    pub fn uploads(&self) -> PathBuf {
        self.data.join("uploads")
    }

    pub fn spill(&self) -> PathBuf {
        self.cache.join("duckdb-tmp")
    }

    pub fn columnar(&self) -> PathBuf {
        self.cache.join("columnar")
    }

    pub fn projects_file(&self) -> PathBuf {
        self.data.join("projects.json")
    }

    pub fn registry_file(&self) -> PathBuf {
        self.data.join("registry.json")
    }
}

/// Creates the spill folder and points DuckDB's `temp_directory` at it.
pub fn configure_spill(conn: &Connection, dirs: &Dirs) -> Result<(), EngineError> {
    let spill = dirs.spill();
    fs::create_dir_all(&spill)?;
    let path = spill.to_string_lossy();
    conn.execute_batch(&format!("SET temp_directory = {}", sql_string(&path)))?;
    Ok(())
}

/// Restricts file access to `paths`, then turns external access off for good.
pub fn lock_down_paths(conn: &Connection, paths: &[String]) -> Result<(), EngineError> {
    lock_down(conn, "allowed_paths", paths)
}

/// Restricts file access to everything under `dirs`, then turns external access off for good.
pub fn lock_down_dirs(conn: &Connection, dirs: &[String]) -> Result<(), EngineError> {
    lock_down(conn, "allowed_directories", dirs)
}

fn lock_down(conn: &Connection, setting: &str, allowed: &[String]) -> Result<(), EngineError> {
    if !allowed.is_empty() {
        let list = allowed
            .iter()
            .map(|item| sql_string(item))
            .collect::<Vec<_>>()
            .join(", ");
        conn.execute_batch(&format!("SET {setting} = [{list}]"))?;
    }
    conn.execute_batch("SET enable_external_access = false")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::scan_expression;
    use std::path::Path;

    fn count_rows(conn: &Connection, file: &Path) -> duckdb::Result<i64> {
        let scan = scan_expression(&file.to_string_lossy()).unwrap();
        conn.query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
            row.get(0)
        })
    }

    #[test]
    fn spill_folder_is_configured() {
        let root = tempfile::tempdir().unwrap();
        let dirs = Dirs {
            data: root.path().join("data"),
            cache: root.path().join("cache"),
        };
        let conn = Connection::open_in_memory().unwrap();

        configure_spill(&conn, &dirs).unwrap();

        assert!(dirs.spill().is_dir());
        let setting: String = conn
            .query_row("SELECT current_setting('temp_directory')", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            fs::canonicalize(setting).unwrap(),
            fs::canonicalize(dirs.spill()).unwrap()
        );
    }

    #[test]
    fn dirs_name_the_expected_files() {
        let dirs = Dirs {
            data: PathBuf::from("/d"),
            cache: PathBuf::from("/c"),
        };
        assert_eq!(dirs.uploads(), Path::new("/d/uploads"));
        assert_eq!(dirs.spill(), Path::new("/c/duckdb-tmp"));
        assert_eq!(dirs.columnar(), Path::new("/c/columnar"));
        assert_eq!(dirs.projects_file(), Path::new("/d/projects.json"));
        assert_eq!(dirs.registry_file(), Path::new("/d/registry.json"));
    }

    #[test]
    fn lockdown_allows_only_listed_files() {
        let root = tempfile::tempdir().unwrap();
        let allowed = root.path().join("it's.csv");
        let other = root.path().join("other.csv");
        fs::write(&allowed, "a\n1\n2\n").unwrap();
        fs::write(&other, "a\n1\n").unwrap();
        let conn = Connection::open_in_memory().unwrap();

        lock_down_paths(&conn, &[allowed.to_string_lossy().into_owned()]).unwrap();

        assert_eq!(count_rows(&conn, &allowed).unwrap(), 2);
        assert!(count_rows(&conn, &other).is_err());
    }

    #[test]
    fn lockdown_allows_only_listed_directories() {
        let root = tempfile::tempdir().unwrap();
        let inside = root.path().join("in");
        let outside = root.path().join("out");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(inside.join("a.csv"), "a\n1\n").unwrap();
        fs::write(outside.join("b.csv"), "a\n1\n").unwrap();
        let conn = Connection::open_in_memory().unwrap();

        lock_down_dirs(&conn, &[inside.to_string_lossy().into_owned()]).unwrap();

        assert_eq!(count_rows(&conn, &inside.join("a.csv")).unwrap(), 1);
        assert!(count_rows(&conn, &outside.join("b.csv")).is_err());
    }

    #[test]
    fn lockdown_cannot_be_undone() {
        let conn = Connection::open_in_memory().unwrap();

        lock_down_paths(&conn, &[]).unwrap();

        assert!(
            conn.execute_batch("SET enable_external_access = true")
                .is_err()
        );
        assert!(conn.execute_batch("SET allowed_paths = ['/']").is_err());
    }
}
