//! Project workspaces: one in-memory engine per project, with every source mounted as views.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;
use duckdb::Connection;
use serde::Serialize;

use self::datasets::attached_datasets;
use self::workbook::is_workbook;
use crate::relation::describe::{DatasetInfo, describe};
use crate::runtime::cache::columnar_key::{ColumnarJob, ColumnarKey};
use crate::runtime::cache::responses::source_files;
use crate::runtime::dirs::{Dirs, configure_spill, lock_down_paths};
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::registry::{ProjectRecord, SourceRecord};
use crate::shared::ids::{database_alias, dataset_id, source_schema, view_id};
use crate::shared::sql::{quote_ident, scan_expression, sql_string};

mod datasets;
pub(crate) mod workbook;

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

pub(crate) fn is_database(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("duckdb") || ext.eq_ignore_ascii_case("db"))
}

/// The library version, which is part of every cache key.
pub(crate) fn engine_version() -> &'static str {
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
    Ok(format!("{}.main.data", attach_alias(conn, key, file)?))
}

/// Like `attach_cache`, but returns the quoted alias of the attached cache file.
fn attach_alias(conn: &Connection, key: &ColumnarKey, file: &Path) -> duckdb::Result<String> {
    let alias = quote_ident(&format!("cache_{}", &key.as_str()[..16]));
    conn.execute_batch(&format!(
        "ATTACH IF NOT EXISTS {} AS {alias} (READ_ONLY)",
        sql_string(&file.to_string_lossy())
    ))?;
    Ok(alias)
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
                paths.extend(workbook::allowed_paths(source, &dirs.cache));
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
        let mut engine = Engine::new(kind, conn)?;
        engine.caches = caches;
        engine.cache_dir.clone_from(&dirs.cache);
        engine.source_files = source_files(sources);
        Ok(engine)
    }

    /// Mounts `source` into this workspace, once; later calls return the same views.
    pub fn mount(
        &self,
        project: &ProjectRecord,
        source: &SourceRecord,
    ) -> anyhow::Result<Vec<ViewInfo>> {
        let mut primary = self.primary();
        if let Some(views) = primary.mounted.get(&source.id) {
            return Ok(views.clone());
        }
        let views = mount_source(
            &primary.conn,
            project,
            source,
            &self.caches,
            &self.cache_dir,
        )?;
        primary.mounted.insert(source.id.clone(), views.clone());
        Ok(views)
    }
}

fn mount_source(
    conn: &Connection,
    project: &ProjectRecord,
    source: &SourceRecord,
    caches: &HashMap<ColumnarKey, PathBuf>,
    cache_dir: &Path,
) -> anyhow::Result<Vec<ViewInfo>> {
    if is_workbook(&source.source) {
        return workbook::mount(conn, project, source, cache_dir);
    }
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
mod tests;
