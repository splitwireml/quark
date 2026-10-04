//! Project workspaces: one in-memory engine per project, with every source mounted as views.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;
use duckdb::{Connection, params};
use serde::Serialize;

use crate::cache::columnar::{ColumnarJob, ColumnarKey};
use crate::engine::{
    DatasetInfo, Dirs, Engine, EngineKind, configure_spill, describe, lock_down_paths,
};
use crate::ids::{database_alias, dataset_id, source_schema, view_id};
use crate::registry::{ProjectRecord, SourceRecord};
use crate::sql::{quote_ident, scan_expression, sql_string};

/// One mounted dataset, as listed by the project's views endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViewInfo {
    pub id: String,
    pub project_id: String,
    pub source_id: String,
    pub source_name: String,
    pub node_id: String,
    pub name: String,
    pub schema: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub columns: Vec<String>,
    pub sql: String,
}

fn is_database(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("duckdb") || ext.eq_ignore_ascii_case("db"))
}

/// The library version, which is part of every cache key.
fn engine_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        Connection::open_in_memory()
            .and_then(|conn| conn.version())
            .unwrap_or_default()
    })
}

/// The import that fills the columnar cache of a flat-file source.
/// `None` for databases and for files that cannot be keyed or scanned.
pub(crate) fn import_job(source: &SourceRecord) -> Option<ColumnarJob> {
    if is_database(&source.source) {
        return None;
    }
    let scan_expression = scan_expression(&source.source.to_string_lossy())?;
    let key = ColumnarKey::new(&source.source, &scan_expression, engine_version()).ok()?;
    Some(ColumnarJob {
        source_id: source.id.clone(),
        key,
        source_path: source.source.clone(),
        scan_expression,
    })
}

/// Attaches a cache file read-only and returns the relation that reads its table.
/// Two sources over the same file share one cache, so a repeated attach is a no-op.
pub(crate) fn attach_cache(
    conn: &Connection,
    key: &ColumnarKey,
    file: &Path,
) -> duckdb::Result<String> {
    let alias = quote_ident(&format!("cache_{}", &key.as_str()[..16]));
    conn.execute_batch(&format!(
        "ATTACH IF NOT EXISTS {} AS {alias} (READ_ONLY)",
        sql_string(&file.to_string_lossy())
    ))?;
    Ok(format!("{alias}.main.data"))
}

/// A database file and its write-ahead log, which DuckDB looks for when attaching.
pub(crate) fn database_paths(file: &Path) -> [String; 2] {
    let path = file.to_string_lossy().into_owned();
    [format!("{path}.wal"), path]
}

impl Engine {
    /// Opens a project's workspace: an in-memory database that may read only its sources' files.
    pub fn open_workspace(
        project: &ProjectRecord,
        sources: &[SourceRecord],
        dirs: &Dirs,
        generation: u64,
    ) -> anyhow::Result<Engine> {
        let conn = Connection::open_in_memory()?;
        configure_spill(&conn, dirs)?;
        let mut paths = Vec::with_capacity(sources.len());
        let mut caches = HashMap::new();
        for source in sources {
            if is_database(&source.source) {
                paths.extend(database_paths(&source.source));
            } else {
                paths.push(source.source.to_string_lossy().into_owned());
            }
            if let Some(job) = import_job(source)
                && job.key.is_ready(&dirs.cache)
            {
                let file = job.key.final_path(&dirs.cache);
                paths.extend(database_paths(&file));
                caches.insert(job.key, file);
            }
        }
        paths.sort();
        lock_down_paths(&conn, &paths)?;
        let kind = EngineKind::Workspace {
            project_id: project.id.clone(),
            node_id: project.node_id.clone(),
        };
        let mut engine = Engine::new(kind, generation, conn);
        engine.caches = caches;
        Ok(engine)
    }

    /// Mounts `source` into this workspace, once; later calls return the same views.
    pub fn mount(
        &self,
        project: &ProjectRecord,
        source: &SourceRecord,
    ) -> anyhow::Result<Vec<ViewInfo>> {
        let mut inner = self.lock();
        if let Some(views) = inner.mounted.get(&source.id) {
            return Ok(views.clone());
        }
        let views = mount_source(&inner.conn, project, source, &self.caches)?;
        inner.mounted.insert(source.id.clone(), views.clone());
        Ok(views)
    }
}

fn mount_source(
    conn: &Connection,
    project: &ProjectRecord,
    source: &SourceRecord,
    caches: &HashMap<ColumnarKey, PathBuf>,
) -> anyhow::Result<Vec<ViewInfo>> {
    let path = source.source.to_string_lossy();
    if is_database(&source.source) {
        let alias = database_alias(&source.id);
        conn.execute_batch(&format!(
            "ATTACH {} AS {} (READ_ONLY)",
            sql_string(&path),
            quote_ident(&alias)
        ))?;
        attached_datasets(conn, &alias)?
            .iter()
            .map(|item| {
                let origin = format!(
                    "{}.{}.{}",
                    quote_ident(&alias),
                    quote_ident(&item.schema),
                    quote_ident(&item.name)
                );
                mount_dataset(conn, project, source, item, &origin)
            })
            .collect()
    } else {
        let scan = scan_expression(&path).ok_or_else(|| anyhow!("unsupported file type"))?;
        let job = import_job(source);
        let cached = job
            .as_ref()
            .and_then(|job| caches.get(&job.key).map(|file| (&job.key, file)));
        let origin = match cached {
            Some((key, file)) => attach_cache(conn, key, file)?,
            None => scan,
        };
        let name = source.dataset_name.as_deref().unwrap_or("data");
        let item = DatasetInfo {
            id: dataset_id("main", name),
            name: name.to_owned(),
            schema: "main".to_owned(),
            kind: "VIEW".to_owned(),
        };
        Ok(vec![mount_dataset(conn, project, source, &item, &origin)?])
    }
}

/// Tables and views of the database attached as `alias`, ordered by schema then name.
fn attached_datasets(conn: &Connection, alias: &str) -> duckdb::Result<Vec<DatasetInfo>> {
    let mut statement = conn.prepare(
        "SELECT schema_name, table_name, 'TABLE' FROM duckdb_tables()
         WHERE database_name = ? AND NOT internal
           AND schema_name NOT IN ('information_schema', 'pg_catalog')
         UNION ALL
         SELECT schema_name, view_name, 'VIEW' FROM duckdb_views()
         WHERE database_name = ? AND NOT internal
           AND schema_name NOT IN ('information_schema', 'pg_catalog')
         ORDER BY 1, 2",
    )?;
    statement
        .query_map(params![alias, alias], |row| {
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

/// Creates the dataset's own schema holding one view over `origin`.
fn mount_dataset(
    conn: &Connection,
    project: &ProjectRecord,
    source: &SourceRecord,
    item: &DatasetInfo,
    origin: &str,
) -> anyhow::Result<ViewInfo> {
    let schema = quote_ident(&source_schema(&source.id, &item.id));
    let target = format!("{schema}.{}", quote_ident(&item.name));
    conn.execute_batch(&format!("CREATE SCHEMA {schema}"))?;
    conn.execute_batch(&format!("CREATE VIEW {target} AS SELECT * FROM {origin}"))?;
    let columns = describe(conn, &target, &[])?
        .into_iter()
        .map(|column| column.name)
        .collect();
    Ok(ViewInfo {
        id: view_id(&project.id, &source.id, &item.id),
        project_id: project.id.clone(),
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        node_id: project.node_id.clone(),
        name: item.name.clone(),
        schema: item.schema.clone(),
        kind: item.kind.clone(),
        columns,
        sql: format!("SELECT * FROM {target}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const DATA_SCHEMA: &str = "source_WyJzMSIsIld5SnRZV2x1SWl3aVpHRjBZU0pkIl0";

    fn dirs_in(root: &Path) -> Dirs {
        Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        }
    }

    fn project() -> ProjectRecord {
        ProjectRecord {
            id: "p1".to_owned(),
            name: "P1".to_owned(),
            node_id: "project_p1".to_owned(),
        }
    }

    fn record(file: &Path, dataset_name: Option<&str>) -> SourceRecord {
        SourceRecord {
            id: "s1".to_owned(),
            name: "Claims".to_owned(),
            kind: "file".to_owned(),
            source: file.to_path_buf(),
            project_id: Some("p1".to_owned()),
            dataset_name: dataset_name.map(str::to_owned),
            sheets: None,
        }
    }

    fn workspace(source: &SourceRecord, root: &Path) -> Engine {
        Engine::open_workspace(&project(), std::slice::from_ref(source), &dirs_in(root), 1).unwrap()
    }

    fn csv(root: &Path, name: &str) -> std::path::PathBuf {
        let file = root.join(name);
        fs::write(&file, "a,b\n1,x\n2,y\n").unwrap();
        file
    }

    #[test]
    fn workspace_views_have_python_ids_and_sql() {
        let root = tempfile::tempdir().unwrap();
        let source = record(&csv(root.path(), "claims.csv"), None);
        let engine = workspace(&source, root.path());

        let views = engine.mount(&project(), &source).unwrap();

        let sql = format!("SELECT * FROM \"{DATA_SCHEMA}\".\"data\"");
        assert_eq!(
            views,
            [ViewInfo {
                id: "WyJwMSIsInMxIiwiV3lKdFlXbHVJaXdpWkdGMFlTSmQiXQ".to_owned(),
                project_id: "p1".to_owned(),
                source_id: "s1".to_owned(),
                source_name: "Claims".to_owned(),
                node_id: "project_p1".to_owned(),
                name: "data".to_owned(),
                schema: "main".to_owned(),
                kind: "VIEW".to_owned(),
                columns: vec!["a".to_owned(), "b".to_owned()],
                sql: sql.clone(),
            }]
        );
        let json = serde_json::to_value(&views[0]).unwrap();
        assert_eq!(json["type"], "VIEW");
        assert!(json.get("kind").is_none());
        let rows: i64 = engine
            .lock()
            .conn
            .query_row(&format!("SELECT count(*) FROM ({sql})"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 2);
        assert_eq!(
            engine.kind,
            EngineKind::Workspace {
                project_id: "p1".to_owned(),
                node_id: "project_p1".to_owned(),
            }
        );
    }

    #[test]
    fn mount_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let source = record(&csv(root.path(), "claims.csv"), Some("claims"));
        let engine = workspace(&source, root.path());

        let first = engine.mount(&project(), &source).unwrap();
        let second = engine.mount(&project(), &source).unwrap();

        assert_eq!(first, second);
        assert_eq!(engine.lock().mounted.len(), 1);
    }

    #[test]
    fn duckdb_sources_mount_every_table_read_only() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("x.duckdb");
        Connection::open(&database)
            .unwrap()
            .execute_batch(
                "CREATE TABLE items (a INTEGER);
                 INSERT INTO items VALUES (1), (2);
                 CREATE VIEW v AS SELECT a FROM items;
                 CREATE SCHEMA other;
                 CREATE TABLE other.t (b VARCHAR, c DOUBLE)",
            )
            .unwrap();
        let source = record(&database, None);
        let engine = workspace(&source, root.path());

        let views = engine.mount(&project(), &source).unwrap();

        let summary: Vec<_> = views
            .iter()
            .map(|view| (view.schema.as_str(), view.name.as_str(), view.kind.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("main", "items", "TABLE"),
                ("main", "v", "VIEW"),
                ("other", "t", "TABLE")
            ]
        );
        assert_eq!(views[2].columns, ["b", "c"]);
        let guard = engine.lock();
        let rows: i64 = guard
            .conn
            .query_row(
                &views[0].sql.replace("SELECT *", "SELECT count(*)"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 2);
        let alias = quote_ident(&database_alias("s1"));
        assert!(
            guard
                .conn
                .execute_batch(&format!("INSERT INTO {alias}.main.items VALUES (3)"))
                .is_err()
        );
        assert!(
            guard
                .conn
                .execute_batch(&format!("CREATE TABLE {alias}.main.more (a INTEGER)"))
                .is_err()
        );
    }

    #[test]
    fn workspace_cannot_read_unlisted_files() {
        let root = tempfile::tempdir().unwrap();
        let listed = csv(root.path(), "listed.csv");
        let unlisted = csv(root.path(), "unlisted.csv");
        let source = record(&listed, None);
        let engine = workspace(&source, root.path());
        let rows = |file: &Path| {
            let scan = scan_expression(&file.to_string_lossy()).unwrap();
            engine
                .lock()
                .conn
                .query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
                    row.get::<_, i64>(0)
                })
        };

        assert_eq!(rows(&listed).unwrap(), 2);
        assert!(rows(&unlisted).is_err());

        let empty = Engine::open_workspace(&project(), &[], &dirs_in(root.path()), 1).unwrap();
        let scan = scan_expression(&listed.to_string_lossy()).unwrap();
        assert!(
            empty
                .lock()
                .conn
                .query_row(&format!("SELECT count(*) FROM {scan}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .is_err()
        );
    }

    #[test]
    fn workspace_allow_list_is_exact() {
        let root = tempfile::tempdir().unwrap();
        let flat = record(&csv(root.path(), "claims.csv"), None);
        let database = root.path().join("x.duckdb");
        Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE items (a INTEGER)")
            .unwrap();
        let other = SourceRecord {
            id: "s2".to_owned(),
            source: database.clone(),
            ..record(&database, None)
        };
        let key = build_cache(&flat, root.path());
        let cache = key.final_path(&dirs_in(root.path()).cache);
        let relative = |file: &str| {
            let name = root.path().file_name().unwrap().to_str().unwrap();
            let file = file.replace('\\', "/");
            file.split_once(name).unwrap().1.to_owned()
        };
        let mut expected: Vec<String> = [
            flat.source.to_string_lossy().into_owned(),
            database.to_string_lossy().into_owned(),
            format!("{}.wal", database.display()),
            cache.to_string_lossy().into_owned(),
            format!("{}.wal", cache.display()),
        ]
        .iter()
        .map(|file| relative(file))
        .collect();
        expected.sort();

        let sources = [flat, other];
        let engine =
            Engine::open_workspace(&project(), &sources, &dirs_in(root.path()), 1).unwrap();
        let mut allowed: Vec<String> = engine
            .lock()
            .conn
            .prepare("SELECT unnest(current_setting('allowed_paths'))")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|file| relative(&file.unwrap()))
            .collect();
        // DuckDB keeps the list as a set, so only its contents are observable.
        allowed.sort();

        assert_eq!(allowed, expected);
    }

    fn build_cache(source: &SourceRecord, root: &Path) -> ColumnarKey {
        let job = import_job(source).unwrap();
        let dirs = dirs_in(root);
        fs::create_dir_all(dirs.columnar()).unwrap();
        Connection::open(job.key.final_path(&dirs.cache))
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE data AS SELECT * FROM {}",
                job.scan_expression
            ))
            .unwrap();
        job.key
    }

    fn rows(engine: &Engine, view_sql: &str) -> Vec<String> {
        engine
            .lock()
            .conn
            .prepare(&format!(
                "SELECT CAST(t AS VARCHAR) FROM ({view_sql}) t ORDER BY 1"
            ))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn definition(engine: &Engine) -> String {
        engine
            .lock()
            .conn
            .query_row(
                "SELECT sql FROM duckdb_views() WHERE NOT internal AND view_name = 'data'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn alias(key: &ColumnarKey) -> String {
        format!("cache_{}", &key.as_str()[..16])
    }

    #[test]
    fn ready_cache_is_attached_read_only_with_identical_rows() {
        let root = tempfile::tempdir().unwrap();
        let source = record(&csv(root.path(), "claims.csv"), None);
        let live = workspace(&source, root.path());
        let live_views = live.mount(&project(), &source).unwrap();
        let key = build_cache(&source, root.path());

        let cached = workspace(&source, root.path());
        let views = cached.mount(&project(), &source).unwrap();

        assert_eq!(views, live_views);
        assert_eq!(rows(&cached, &views[0].sql), rows(&live, &views[0].sql));
        assert!(definition(&cached).contains(&alias(&key)));
        assert!(!definition(&live).contains("cache_"));
        let insert = format!("INSERT INTO {}.main.data VALUES (3, 'z')", alias(&key));
        assert!(cached.lock().conn.execute_batch(&insert).is_err());
    }

    #[test]
    fn missing_cache_file_mounts_live() {
        let root = tempfile::tempdir().unwrap();
        let source = record(&csv(root.path(), "claims.csv"), None);
        let key = build_cache(&source, root.path());
        fs::remove_file(key.final_path(&dirs_in(root.path()).cache)).unwrap();

        let engine = workspace(&source, root.path());
        let views = engine.mount(&project(), &source).unwrap();

        assert!(!definition(&engine).contains("cache_"));
        assert_eq!(rows(&engine, &views[0].sql).len(), 2);
    }

    #[test]
    fn edited_source_mounts_live_until_it_has_a_new_cache() {
        let root = tempfile::tempdir().unwrap();
        let file = csv(root.path(), "claims.csv");
        let source = record(&file, None);
        build_cache(&source, root.path());
        fs::write(&file, "a,b\n1,x\n2,y\n3,z\n").unwrap();

        let engine = workspace(&source, root.path());
        let views = engine.mount(&project(), &source).unwrap();

        assert!(!definition(&engine).contains("cache_"));
        assert_eq!(rows(&engine, &views[0].sql).len(), 3);
    }

    #[test]
    fn engines_with_a_ready_cache_list_only_their_view() {
        let root = tempfile::tempdir().unwrap();
        let source = record(&csv(root.path(), "claims.csv"), None);
        let key = build_cache(&source, root.path());
        let listed = |engine: &Engine| -> Vec<(String, String)> {
            crate::engine::datasets(&engine.lock().conn)
                .unwrap()
                .into_iter()
                .map(|item| (item.name, item.kind))
                .collect()
        };
        let only_view = [("data".to_owned(), "VIEW".to_owned())];

        let node = Engine::open_node(&source, &dirs_in(root.path()), 1).unwrap();
        let workspace = workspace(&source, root.path());
        workspace.mount(&project(), &source).unwrap();

        assert!(definition(&node).contains(&alias(&key)));
        assert!(definition(&workspace).contains(&alias(&key)));
        assert_eq!(listed(&node), only_view);
        assert_eq!(listed(&workspace), only_view);
        assert_eq!(rows(&node, "SELECT * FROM data").len(), 2);
    }

    #[test]
    fn only_flat_files_have_an_import_job() {
        let root = tempfile::tempdir().unwrap();
        let flat = record(&csv(root.path(), "claims.csv"), None);
        let job = import_job(&flat).unwrap();
        assert_eq!(job.source_id, "s1");
        assert_eq!(job.source_path, flat.source);
        assert!(job.scan_expression.starts_with("read_csv_auto("));
        for name in ["x.duckdb", "x.db", "x.xlsx", "x.txt"] {
            let file = root.path().join(name);
            fs::write(&file, "x").unwrap();
            assert!(import_job(&record(&file, None)).is_none(), "{name}");
        }
        assert!(import_job(&record(&root.path().join("gone.csv"), None)).is_none());
    }
}
