//! The engines: a DuckDB connection with its mounted views and caches, and how a source opens one.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::{Context, bail};
use duckdb::{AccessMode, Config, Connection, InterruptHandle};

use super::cache::columnar_key::ColumnarKey;
use super::cache::results::ResultCache;
use super::cache::stats::StatsCache;
use super::dirs::{Dirs, configure_spill, lock_down_dirs};
use super::mount::{ViewInfo, attach_cache, import_job};
use super::registry::SourceRecord;
use crate::shared::sql::{quote_ident, scan_expression};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineKind {
    Workspace { project_id: String, node_id: String },
    Node { node_id: String },
}

/// Work done so far, so tests can tell a cache hit from a recomputation.
#[derive(Debug, Default)]
pub struct Counters {
    pub count_queries: u64,
}

pub struct EngineInner {
    pub conn: Connection,
    /// Views mounted so far, by source id.
    pub mounted: BTreeMap<String, Vec<ViewInfo>>,
    pub stats: StatsCache,
    pub results: ResultCache,
    #[doc(hidden)]
    pub counters: Counters,
}

impl EngineInner {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            mounted: BTreeMap::new(),
            stats: StatsCache::default(),
            results: ResultCache::default(),
            counters: Counters::default(),
        }
    }
}

pub struct Engine {
    pub kind: EngineKind,
    pub generation: u64,
    // ponytail: one query at a time per engine; parallel reads are the upgrade path (sub-project 2).
    inner: Mutex<EngineInner>,
    pub(crate) interrupt: Arc<InterruptHandle>,
    /// Ticket of the query running now, if any (see `cancel`).
    pub(crate) active: Mutex<Option<u64>>,
    pub(crate) next_ticket: AtomicU64,
    /// Cache files that were ready when this workspace opened, so its allow-list covers them.
    pub(crate) caches: HashMap<ColumnarKey, PathBuf>,
}

impl Engine {
    pub fn new(kind: EngineKind, generation: u64, conn: Connection) -> Self {
        Self {
            kind,
            generation,
            interrupt: conn.interrupt_handle(),
            inner: Mutex::new(EngineInner::new(conn)),
            active: Mutex::new(None),
            next_ticket: AtomicU64::new(0),
            caches: HashMap::new(),
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
                let parent = source.source.parent().unwrap_or(&source.source);
                lock_down_dirs(&conn, &[parent.to_string_lossy().into_owned()])?;
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
                let origin = match import_job(source).filter(|job| job.key.is_ready(&dirs.cache)) {
                    Some(job) => {
                        let file = job.key.final_path(&dirs.cache);
                        attach_cache(&conn, &job.key, &file).context("could not read cache")?
                    }
                    None => scan,
                };
                conn.execute_batch(&format!("CREATE VIEW {name} AS SELECT * FROM {origin}"))
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

#[cfg(test)]
mod tests;
