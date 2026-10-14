//! The engines: a DuckDB connection with its mounted views and caches, and how a source opens one.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use anyhow::{Context, bail};
use duckdb::{AccessMode, Config, Connection, InterruptHandle};

use super::cache::columnar_key::ColumnarKey;
use super::cache::numbered::NumberedCache;
use super::cache::responses::{ResponseCache, source_files};
use super::cache::results::ResultCache;
use super::cache::stats::StatsCache;
use super::dirs::{Dirs, configure_spill, lock_down_dirs};
use super::mount::workbook::open_node as workbook_node;
use super::mount::{ViewInfo, attach_cache, import_job};
use super::pool::{ReaderPool, reader_count};
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
    pub count_queries: AtomicU64,
}

/// The connection that changes the catalog, and the views mounted through it by source id.
pub struct Primary {
    pub conn: Connection,
    pub mounted: BTreeMap<String, Vec<ViewInfo>>,
}

pub struct Engine {
    pub kind: EngineKind,
    primary: Mutex<Primary>,
    /// Readers cloned from the primary: queries run here, in parallel (see `cancel`).
    pub(crate) pool: ReaderPool,
    pub stats: Mutex<StatsCache>,
    pub results: Mutex<ResultCache>,
    pub(crate) numbered: Mutex<NumberedCache>,
    pub(crate) responses: Mutex<ResponseCache>,
    /// Functions and macros that make a response uncacheable, read from the catalog on first use.
    pub(crate) volatile: OnceLock<HashSet<String>>,
    /// Every file this engine reads, so a response is only reused while none of them changed.
    pub(crate) source_files: Vec<PathBuf>,
    #[doc(hidden)]
    pub counters: Counters,
    /// Tickets of the queries running now, with the handle that interrupts each one's reader.
    pub(crate) active: Mutex<HashMap<u64, Arc<InterruptHandle>>>,
    pub(crate) next_ticket: AtomicU64,
    /// Cache files that were ready when this workspace opened, so its allow-list covers them.
    pub(crate) caches: HashMap<ColumnarKey, PathBuf>,
    /// The cache folder, where a workspace imports a workbook that has no cache yet.
    pub(crate) cache_dir: PathBuf,
}

/// Locks `mutex`, recovering its data if a panicking thread poisoned it.
pub fn lock_recovering<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Engine {
    pub fn new(kind: EngineKind, conn: Connection) -> duckdb::Result<Self> {
        let pool = ReaderPool::new(&conn, reader_count())?;
        Ok(Self {
            kind,
            primary: Mutex::new(Primary {
                conn,
                mounted: BTreeMap::new(),
            }),
            pool,
            stats: Mutex::new(StatsCache::default()),
            results: Mutex::new(ResultCache::default()),
            numbered: Mutex::new(NumberedCache::default()),
            responses: Mutex::new(ResponseCache::default()),
            volatile: OnceLock::new(),
            source_files: Vec::new(),
            counters: Counters::default(),
            active: Mutex::new(HashMap::new()),
            next_ticket: AtomicU64::new(0),
            caches: HashMap::new(),
            cache_dir: PathBuf::new(),
        })
    }

    /// The primary connection, for catalog changes only: mounting, attaching, dropping result tables.
    pub fn primary(&self) -> MutexGuard<'_, Primary> {
        lock_recovering(&self.primary)
    }

    /// The count and null cache; hold the guard only to read or write one entry.
    pub fn stats(&self) -> MutexGuard<'_, StatsCache> {
        lock_recovering(&self.stats)
    }

    /// The result cache; hold the guard only to read or write one entry.
    pub fn results(&self) -> MutexGuard<'_, ResultCache> {
        lock_recovering(&self.results)
    }

    /// The numbered tables find searches read; hold the guard only to read or write one entry.
    pub fn numbered(&self) -> MutexGuard<'_, NumberedCache> {
        lock_recovering(&self.numbered)
    }

    /// Opens a source on its own connection: databases read-only, flat files behind one view.
    pub fn open_node(source: &SourceRecord, dirs: &Dirs) -> anyhow::Result<Engine> {
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
                lock_down_dirs(&conn, &[])?;
                conn
            }
            Some("xlsx") => workbook_node(source, dirs)?,
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
        let mut engine = Engine::new(kind, conn)?;
        engine.source_files = source_files(std::slice::from_ref(source));
        Ok(engine)
    }
}

#[cfg(test)]
mod tests;
