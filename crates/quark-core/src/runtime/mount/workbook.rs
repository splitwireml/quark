//! Workbook sources: the typed cache each one reads, and how nodes and workspaces open it.

use std::path::{Path, PathBuf};

use duckdb::Connection;

use super::datasets::sheet_datasets;
use super::{ViewInfo, attach_alias, database_paths, mount_dataset};
use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::cache::workbook::sheet_names;
use crate::runtime::cache::workbook_import::{import_workbook, workbook_key};
use crate::runtime::dirs::{Dirs, configure_spill, lock_down_dirs};
use crate::runtime::registry::{ProjectRecord, SourceRecord};
use crate::shared::sql::quote_ident;

pub(crate) fn is_workbook(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx"))
}

/// The sheets the user picked, or every sheet of the workbook.
fn sheets_of(source: &SourceRecord) -> anyhow::Result<Vec<String>> {
    match &source.sheets {
        Some(sheets) => Ok(sheets.clone()),
        None => sheet_names(&source.source),
    }
}

/// The source's cache file and key, importing the workbook first when the cache is missing or evicted.
fn ready_cache(
    source: &SourceRecord,
    cache_dir: &Path,
) -> anyhow::Result<(ColumnarKey, PathBuf, Vec<String>)> {
    let sheets = sheets_of(source)?;
    let file = import_workbook(&source.source, &sheets, cache_dir)?;
    let key = workbook_key(&source.source, &sheets)?;
    key.touch(cache_dir);
    Ok((key, file, sheets))
}

/// The cache file and its log, which a workspace must allow before it can attach them.
pub(super) fn allowed_paths(source: &SourceRecord, cache_dir: &Path) -> Vec<String> {
    if !is_workbook(&source.source) {
        return Vec::new();
    }
    sheets_of(source)
        .ok()
        .and_then(|sheets| workbook_key(&source.source, &sheets).ok())
        .map(|key| database_paths(&key.final_path(cache_dir)).into())
        .unwrap_or_default()
}

/// Opens a workbook node: every sheet copied from the cache into a table of its own.
pub(crate) fn open_node(source: &SourceRecord, dirs: &Dirs) -> anyhow::Result<Connection> {
    let (key, file, sheets) = ready_cache(source, &dirs.cache)?;
    let conn = Connection::open_in_memory()?;
    configure_spill(&conn, dirs)?;
    let alias = attach_alias(&conn, &key, &file)?;
    for sheet in &sheets {
        let table = quote_ident(sheet);
        conn.execute_batch(&format!(
            "CREATE TABLE main.{table} AS SELECT * FROM {alias}.main.{table}"
        ))?;
    }
    conn.execute_batch(&format!("DETACH {alias}"))?;
    let parent = source.source.parent().unwrap_or(&source.source);
    lock_down_dirs(&conn, &[parent.to_string_lossy().into_owned()])?;
    Ok(conn)
}

/// Attaches the workbook's typed cache and returns the quoted origin of `sheet` in it, with the
/// cache file a workspace must also allow along with its log.
pub(crate) fn attach_sheet(
    conn: &Connection,
    source: &SourceRecord,
    sheet: &str,
    cache_dir: &Path,
) -> anyhow::Result<(String, PathBuf)> {
    let (key, file, _) = ready_cache(source, cache_dir)?;
    let alias = attach_alias(conn, &key, &file)?;
    Ok((format!("{alias}.main.{}", quote_ident(sheet)), file))
}

/// Mounts each sheet as a view in its own schema over the attached cache.
pub(super) fn mount(
    conn: &Connection,
    project: &ProjectRecord,
    source: &SourceRecord,
    cache_dir: &Path,
) -> anyhow::Result<Vec<ViewInfo>> {
    let (key, file, sheets) = ready_cache(source, cache_dir)?;
    let alias = attach_alias(conn, &key, &file)?;
    sheet_datasets(&sheets)
        .iter()
        .map(|item| {
            let origin = format!("{alias}.main.{}", quote_ident(&item.name));
            mount_dataset(conn, project, source, item, &origin)
        })
        .collect()
}
