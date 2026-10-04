//! Data and cache folders, spill configuration and DuckDB lockdown.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use anyhow::{Context, bail};
use duckdb::{AccessMode, Config, Connection, ToSql};
use serde::Serialize;

use crate::ids::dataset_id;
use crate::query::ColumnMeta;
use crate::registry::SourceRecord;
use crate::sql::{quote_ident, scan_expression, sql_string};

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

/// One table or view in an engine, as listed by `GET /datasets`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub schema: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineKind {
    Workspace { project_id: String, node_id: String },
    Node { node_id: String },
}

pub struct EngineInner {
    pub conn: Connection,
}

pub struct Engine {
    pub kind: EngineKind,
    pub generation: u64,
    // ponytail: one query at a time per engine; parallel reads are the upgrade path (sub-project 2).
    inner: Mutex<EngineInner>,
}

impl Engine {
    pub fn new(kind: EngineKind, generation: u64, conn: Connection) -> Self {
        Self {
            kind,
            generation,
            inner: Mutex::new(EngineInner { conn }),
        }
    }

    /// Locks the connection, recovering it if a panicking thread poisoned the mutex.
    pub fn lock(&self) -> MutexGuard<'_, EngineInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Opens a source on its own connection: databases read-only, flat files behind one view.
    pub fn open_node(
        source: &SourceRecord,
        dirs: &Dirs,
        generation: u64,
    ) -> anyhow::Result<Engine> {
        let extension = source
            .source
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
        let conn = match extension.as_deref() {
            Some("duckdb" | "db") => {
                let config = Config::default().access_mode(AccessMode::ReadOnly)?;
                let conn = Connection::open_with_flags(&source.source, config)
                    .context("could not open database")?;
                configure_spill(&conn, dirs)?;
                conn.execute_batch("SET enable_external_access = false")?;
                conn
            }
            Some("xlsx") => bail!("Excel sources are not in the desktop build yet"),
            _ => {
                let path = source.source.to_string_lossy();
                let Some(scan) = scan_expression(&path) else {
                    bail!("unsupported file type");
                };
                let conn = Connection::open_in_memory()?;
                configure_spill(&conn, dirs)?;
                let name = quote_ident(source.dataset_name.as_deref().unwrap_or("data"));
                conn.execute_batch(&format!("CREATE VIEW {name} AS SELECT * FROM {scan}"))
                    .context("could not read source")?;
                let parent = source.source.parent().unwrap_or(&source.source);
                lock_down_dirs(&conn, &[parent.to_string_lossy().into_owned()])?;
                conn
            }
        };
        let kind = EngineKind::Node {
            node_id: source.id.clone(),
        };
        Ok(Engine::new(kind, generation, conn))
    }
}

/// Lists user tables and views, ordered by schema then name.
pub fn datasets(conn: &Connection) -> duckdb::Result<Vec<DatasetInfo>> {
    let mut statement = conn.prepare(
        "SELECT schema_name, table_name, 'TABLE' FROM duckdb_tables()
         WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog')
         UNION ALL
         SELECT schema_name, view_name, 'VIEW' FROM duckdb_views()
         WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog')
         ORDER BY 1, 2",
    )?;
    statement
        .query_map([], |row| {
            let schema: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok(DatasetInfo {
                id: dataset_id(&schema, &name),
                name,
                schema,
                kind: row.get(2)?,
            })
        })?
        .collect()
}

/// Column names and types of `relation`, which may contain `?` placeholders bound from `params`.
pub fn describe(
    conn: &Connection,
    relation: &str,
    params: &[&dyn ToSql],
) -> duckdb::Result<Vec<ColumnMeta>> {
    let mut statement = conn.prepare(&format!("DESCRIBE SELECT * FROM {relation}"))?;
    statement
        .query_map(params, |row| {
            Ok(ColumnMeta {
                name: row.get(0)?,
                type_name: row.get(1)?,
            })
        })?
        .collect()
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

    fn dirs_in(root: &Path) -> Dirs {
        Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        }
    }

    fn record(file: &Path, dataset_name: Option<&str>) -> SourceRecord {
        SourceRecord {
            id: "s1".to_owned(),
            name: "s1".to_owned(),
            kind: "file".to_owned(),
            source: file.to_path_buf(),
            project_id: None,
            dataset_name: dataset_name.map(str::to_owned),
            sheets: None,
        }
    }

    fn open(file: &Path, dataset_name: Option<&str>, root: &Path) -> anyhow::Result<Engine> {
        Engine::open_node(&record(file, dataset_name), &dirs_in(root), 1)
    }

    fn names(engine: &Engine) -> Vec<(String, String, String)> {
        datasets(&engine.lock().conn)
            .unwrap()
            .into_iter()
            .map(|item| (item.schema, item.name, item.kind))
            .collect()
    }

    fn main_view(name: &str) -> Vec<(String, String, String)> {
        vec![("main".to_owned(), name.to_owned(), "VIEW".to_owned())]
    }

    #[test]
    fn node_datasets_per_format() {
        let root = tempfile::tempdir().unwrap();
        let json = r#"[{"a":1}]"#;
        let lines = "{\"a\":1}\n";
        for (file, body) in [
            ("x.csv", "a\n1\n"),
            ("x.tsv", "a\n1\n"),
            ("x.json", json),
            ("x.ndjson", lines),
            ("x.jsonl", lines),
        ] {
            let path = root.path().join(file);
            fs::write(&path, body).unwrap();
            let engine = open(&path, Some("x"), root.path()).unwrap();
            assert_eq!(names(&engine), main_view("x"), "{file}");
        }
        let parquet = root.path().join("x.parquet");
        Connection::open_in_memory()
            .unwrap()
            .execute_batch(&format!(
                "COPY (SELECT 1 AS a) TO {} (FORMAT parquet)",
                sql_string(&parquet.to_string_lossy())
            ))
            .unwrap();
        let engine = open(&parquet, Some("x"), root.path()).unwrap();
        assert_eq!(names(&engine), main_view("x"));

        let database = root.path().join("x.duckdb");
        Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE items (a INTEGER)")
            .unwrap();
        let engine = open(&database, None, root.path()).unwrap();
        assert_eq!(
            names(&engine),
            vec![("main".to_owned(), "items".to_owned(), "TABLE".to_owned())]
        );
        assert_eq!(
            datasets(&engine.lock().conn).unwrap()[0].id,
            dataset_id("main", "items")
        );
    }

    #[test]
    fn duckdb_sources_are_read_only() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("x.db");
        Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE items (a INTEGER)")
            .unwrap();
        let outside = root.path().join("outside.csv");
        fs::write(&outside, "a\n1\n").unwrap();

        let engine = open(&database, None, root.path()).unwrap();
        let guard = engine.lock();

        assert!(
            guard
                .conn
                .execute_batch("INSERT INTO items VALUES (1)")
                .is_err()
        );
        assert!(
            guard
                .conn
                .execute_batch("CREATE TABLE more (a INTEGER)")
                .is_err()
        );
        assert!(count_rows(&guard.conn, &outside).is_err());
    }

    #[test]
    fn paths_with_quotes_spaces_and_unicode() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("it's a \"dir\" zoë📊");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("it's \"q\" ü.csv");
        fs::write(&file, "a\n1\n2\n").unwrap();

        let engine = open(&file, Some("we\"ird 'name'"), root.path()).unwrap();
        let guard = engine.lock();

        let rows: i64 = guard
            .conn
            .query_row("SELECT count(*) FROM \"we\"\"ird 'name'\"", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 2);
        assert_eq!(datasets(&guard.conn).unwrap()[0].name, "we\"ird 'name'");
    }

    #[test]
    fn poisoned_engine_lock_recovers() {
        let engine = std::sync::Arc::new(Engine::new(
            EngineKind::Node {
                node_id: "s1".to_owned(),
            },
            1,
            Connection::open_in_memory().unwrap(),
        ));
        let held = std::sync::Arc::clone(&engine);
        let joined = std::thread::spawn(move || {
            let _guard = held.lock();
            panic!("poison the engine lock");
        })
        .join();
        assert!(joined.is_err());
        assert!(engine.inner.is_poisoned());

        let one: i64 = engine
            .lock()
            .conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .unwrap();

        assert_eq!(one, 1);
    }

    #[test]
    fn open_node_rejects_excel_and_unknown_files() {
        let root = tempfile::tempdir().unwrap();
        assert!(open(&root.path().join("x.xlsx"), None, root.path()).is_err());
        assert!(open(&root.path().join("x.txt"), None, root.path()).is_err());
    }

    #[test]
    fn describe_binds_params_and_reports_types() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER, b VARCHAR)")
            .unwrap();

        let columns = describe(&conn, "t WHERE a > ?", &[&1_i64]).unwrap();

        let pairs: Vec<_> = columns
            .iter()
            .map(|c| (c.name.as_str(), c.type_name.as_str()))
            .collect();
        assert_eq!(pairs, [("a", "INTEGER"), ("b", "VARCHAR")]);
    }
}
