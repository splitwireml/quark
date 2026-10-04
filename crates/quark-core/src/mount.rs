//! Project workspaces: one in-memory engine per project, with every source mounted as views.

use std::path::Path;

use anyhow::anyhow;
use duckdb::{Connection, params};
use serde::Serialize;

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
        for source in sources {
            let path = source.source.to_string_lossy().into_owned();
            if is_database(&source.source) {
                paths.push(format!("{path}.wal"));
            }
            paths.push(path);
        }
        paths.sort();
        lock_down_paths(&conn, &paths)?;
        let kind = EngineKind::Workspace {
            project_id: project.id.clone(),
            node_id: project.node_id.clone(),
        };
        Ok(Engine::new(kind, generation, conn))
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
        let views = mount_source(&inner.conn, project, source)?;
        inner.mounted.insert(source.id.clone(), views.clone());
        Ok(views)
    }
}

fn mount_source(
    conn: &Connection,
    project: &ProjectRecord,
    source: &SourceRecord,
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
        let name = source.dataset_name.as_deref().unwrap_or("data");
        let item = DatasetInfo {
            id: dataset_id("main", name),
            name: name.to_owned(),
            schema: "main".to_owned(),
            kind: "VIEW".to_owned(),
        };
        Ok(vec![mount_dataset(conn, project, source, &item, &scan)?])
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
}
