//! Data and cache folders, spill configuration and DuckDB lockdown.

use std::fs;
use std::io;
use std::path::PathBuf;

use duckdb::Connection;

use crate::shared::sql::sql_string;

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
mod tests;
