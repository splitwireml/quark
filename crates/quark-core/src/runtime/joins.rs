//! Cross-source join workspaces: engines that mount two sources side by side, and the few that stay open.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::slice;
use std::sync::Arc;

use anyhow::Context;
use duckdb::Connection;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::relation::describe::DatasetInfo;
use crate::runtime::Dirs;
use crate::runtime::cache::responses::source_files;
use crate::runtime::dirs::{configure_spill, lock_down_paths};
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::mount::workbook::{attach_sheet, is_workbook};
use crate::runtime::mount::{attach_cache, database_paths, import_job, is_database};
use crate::runtime::registry::SourceRecord;
use crate::runtime::state::{AppState, invalid_value};
use crate::shared::sql::{quote_ident, scan_expression, sql_string};

/// How many join engines stay open at once.
pub(crate) const JOIN_LIMIT: usize = 4;

/// The open join engines by id, least recently used first.
#[derive(Default)]
pub(crate) struct JoinSlots {
    engines: VecDeque<(String, Arc<Engine>)>,
}

impl JoinSlots {
    /// The engine for `id`, now the most recent.
    pub(crate) fn get(&mut self, id: &str) -> Option<Arc<Engine>> {
        let position = self.engines.iter().position(|(key, _)| key == id)?;
        let entry = self.engines.remove(position)?;
        let engine = Arc::clone(&entry.1);
        self.engines.push_back(entry);
        Some(engine)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.engines.len()
    }

    /// Keeps `engine` as the most recent and returns the one pushed out, which the caller drops
    /// outside any lock. Queries still running on it hold their own `Arc` and finish.
    pub(crate) fn insert(&mut self, id: String, engine: Arc<Engine>) -> Option<Arc<Engine>> {
        self.engines.push_back((id, engine));
        if self.engines.len() > JOIN_LIMIT {
            return self.engines.pop_front().map(|(_, evicted)| evicted);
        }
        None
    }
}

/// What mounting one side adds to the engine's allow-list and fingerprint.
struct Mounted {
    allowed: Vec<String>,
    files: Vec<PathBuf>,
}

impl AppState {
    /// Opens a fresh engine with `left` mounted under `left_source` and `right` under `right_source`.
    /// It may read only those sides' files. Nothing keeps it open until `keep_join`.
    pub(crate) fn open_join(
        &self,
        left: (&SourceRecord, &DatasetInfo),
        right: (&SourceRecord, &DatasetInfo),
    ) -> ApiResult<(String, Arc<Engine>)> {
        let dirs = &self.0.dirs;
        let conn = Connection::open_in_memory().map_err(invalid_value)?;
        configure_spill(&conn, dirs).map_err(invalid_value)?;
        let mut allowed = Vec::new();
        let mut files = Vec::new();
        for (schema, (source, item)) in [("left_source", left), ("right_source", right)] {
            let mounted = mount_side(&conn, schema, source, item, dirs)?;
            allowed.extend(mounted.allowed);
            files.extend(mounted.files);
        }
        allowed.sort();
        lock_down_paths(&conn, &allowed).map_err(invalid_value)?;
        let id = format!("join_{}", Uuid::new_v4().simple());
        let kind = EngineKind::Node {
            node_id: id.clone(),
        };
        let mut engine = Engine::new(kind, conn).map_err(invalid_value)?;
        engine.cache_dir.clone_from(&dirs.cache);
        engine.source_files = files;
        Ok((id, Arc::new(engine)))
    }

    /// Keeps a join engine open for later requests, dropping the least recent one past the limit.
    pub(crate) fn keep_join(&self, id: &str, engine: &Arc<Engine>) {
        let evicted = self
            .catalog()
            .joins
            .insert(id.to_owned(), Arc::clone(engine));
        drop(evicted);
    }
}

/// Mounts one dataset as `<schema>.<name>`: a database attached read-only, a workbook's typed
/// cache, a flat file's ready cache, or the file itself.
fn mount_side(
    conn: &Connection,
    schema: &str,
    source: &SourceRecord,
    item: &DatasetInfo,
    dirs: &Dirs,
) -> ApiResult<Mounted> {
    let path = source.source.to_string_lossy();
    let mut mounted = Mounted {
        allowed: Vec::new(),
        files: Vec::new(),
    };
    let origin = if is_database(&source.source) {
        let alias = format!("{schema}_database");
        conn.execute_batch(&format!(
            "ATTACH {} AS {} (READ_ONLY)",
            sql_string(&path),
            quote_ident(&alias)
        ))
        .map_err(invalid_value)?;
        mounted.allowed.extend(database_paths(&source.source));
        mounted.files = source_files(slice::from_ref(source));
        format!(
            "{}.{}.{}",
            quote_ident(&alias),
            quote_ident(&item.schema),
            quote_ident(&item.name)
        )
    } else if is_workbook(&source.source) {
        let (origin, file) = attach_sheet(conn, source, &item.name, &dirs.cache)
            .context("could not read workbook cache")
            .map_err(invalid_value)?;
        mounted.allowed.extend(database_paths(&file));
        mounted.files.push(file);
        origin
    } else if let Some(job) = import_job(source).filter(|job| job.key.is_ready(&dirs.cache)) {
        let file = job.key.final_path(&dirs.cache);
        mounted.allowed.extend(database_paths(&file));
        let origin = attach_cache(conn, &job.key, &file)
            .context("could not read cache")
            .map_err(invalid_value)?;
        mounted.files.push(file);
        origin
    } else {
        let scan = scan_expression(&path)
            .ok_or_else(|| ApiError::not_implemented("Not in the desktop build yet"))?;
        mounted.allowed.push(path.into_owned());
        mounted.files = source_files(slice::from_ref(source));
        scan
    };
    let target = format!("{}.{}", quote_ident(schema), quote_ident(&item.name));
    conn.execute_batch(&format!(
        "CREATE SCHEMA {}; CREATE VIEW {target} AS SELECT * FROM {origin}",
        quote_ident(schema)
    ))
    .map_err(invalid_value)?;
    Ok(mounted)
}

#[cfg(test)]
mod tests;
