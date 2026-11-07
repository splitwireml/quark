//! Per-engine bookkeeping of repeated queries (keys from `stats_key`): the second identical request builds a result table.

use std::fmt::Write;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use duckdb::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::result_build::{self, finish, supersede_builds};
use crate::relation::request::QueryRequest;
use crate::runtime::mount::ViewInfo;

/// Result tables per engine: only Building and Ready entries take a slot.
const CAPACITY: usize = 4;
/// Result tables live in a private in-memory database, so read-only sources can have them too.
/// `(READ_WRITE)` is needed because a read-only main database refuses a default in-memory attach.
pub(super) const MEMORY_DATABASE: &str = "quark_mem";
pub const RESULTS_SCHEMA: &str = "quark_mem.quark_results";
/// Keys seen once, or failed, that are remembered apart from the slots so one-off queries never evict a result.
// ponytail: 64 keys is a guess at a working set of repeated queries; raise it if repeats slip through.
const SEEN_CAPACITY: usize = 64;

/// What to do with a request for a query the cache may have seen before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultAction {
    ServeDirect,
    StartBuild(String),
    ServeFrom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Seen,
    Building,
    Ready,
    Failed,
}

pub(super) type Entries = Vec<(String, State)>;

/// True for the states that hold a result slot.
fn has_slot(state: State) -> bool {
    matches!(state, State::Building | State::Ready)
}

pub struct ResultCache {
    capacity: usize,
    /// Keys with their state, from least to most recently used. The build thread updates it too.
    /// Building and Ready entries are capped at `capacity`; Seen and Failed ones at [`SEEN_CAPACITY`].
    entries: Arc<Mutex<Entries>>,
    dropped: Vec<String>,
    /// The newest build's thread; older builds were interrupted and finish on their own.
    build: Option<JoinHandle<()>>,
}

impl Default for ResultCache {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

/// `r_` followed by the first 8 bytes of the key's SHA-256, in hex.
pub fn table_name(key: &str) -> String {
    let mut name = String::with_capacity(18);
    name.push_str("r_");
    for byte in Sha256::digest(key.as_bytes()).iter().take(8) {
        // Writing to a String cannot fail.
        let _ = write!(name, "{byte:02x}");
    }
    name
}

pub fn drop_statement(table: &str) -> String {
    format!("DROP TABLE IF EXISTS {RESULTS_SCHEMA}.{table}")
}

pub(super) fn lock(entries: &Mutex<Entries>) -> MutexGuard<'_, Entries> {
    entries.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Forgets the least recently used Seen or Failed keys beyond [`SEEN_CAPACITY`].
fn trim_seen(entries: &mut Entries) {
    let mut excess = entries
        .iter()
        .filter(|(_, state)| !has_slot(*state))
        .count()
        .saturating_sub(SEEN_CAPACITY);
    entries.retain(|(_, state)| {
        let is_forgotten = excess > 0 && !has_slot(*state);
        excess -= usize::from(is_forgotten);
        !is_forgotten
    });
}

impl ResultCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Arc::new(Mutex::new(Vec::with_capacity(capacity + SEEN_CAPACITY))),
            dropped: Vec::new(),
            build: None,
        }
    }

    pub fn note_request(&mut self, key: &str) -> ResultAction {
        let shared = Arc::clone(&self.entries);
        let mut entries = lock(&shared);
        let Some(index) = entries.iter().position(|(known, _)| known == key) else {
            entries.push((key.to_owned(), State::Seen));
            trim_seen(&mut entries);
            return ResultAction::ServeDirect;
        };
        let (key, state) = entries.remove(index);
        let (state, action) = match state {
            State::Seen => {
                supersede_builds(&mut entries);
                self.make_room(&mut entries);
                let table = table_name(&key);
                (State::Building, ResultAction::StartBuild(table))
            }
            State::Ready => (State::Ready, ResultAction::ServeFrom(table_name(&key))),
            state => (state, ResultAction::ServeDirect),
        };
        entries.push((key, state));
        trim_seen(&mut entries);
        action
    }

    pub fn mark_ready(&mut self, key: &str) {
        finish(&self.entries, key, State::Ready);
    }

    pub fn mark_failed(&mut self, key: &str) {
        finish(&self.entries, key, State::Failed);
    }

    /// Builds `quark_mem.quark_results.<table>` from `ordered_sql` on a clone of `conn` and its own thread.
    pub fn start_build(
        &mut self,
        conn: &Connection,
        key: &str,
        table: &str,
        ordered_sql: &str,
        params: Vec<Value>,
    ) {
        let build = result_build::start(conn, &self.entries, key, table, ordered_sql, params);
        if build.is_some() {
            self.build = build;
        }
    }

    /// Tables of evicted entries since the last call; run [`drop_statement`] for each.
    pub fn take_dropped(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dropped)
    }

    /// Evicts the least recently used slot entries until one slot is free, noting their tables as dropped.
    /// A Building entry that is evicted is no longer found by its build, which then interrupts itself.
    fn make_room(&mut self, entries: &mut Entries) {
        while entries.iter().filter(|(_, state)| has_slot(*state)).count() >= self.capacity {
            let Some(oldest) = entries.iter().position(|(_, state)| has_slot(*state)) else {
                return;
            };
            let (key, _) = entries.remove(oldest);
            self.dropped.push(table_name(&key));
        }
    }
}

/// Dropping an engine stops its build, and waits until the build's connection is closed.
impl Drop for ResultCache {
    fn drop(&mut self) {
        supersede_builds(&mut lock(&self.entries));
        if let Some(build) = self.build.take()
            && build.join().is_err()
        {
            tracing::warn!("result build thread panicked");
        }
    }
}

/// True when the request has no filters, sorts or dedupe and `sql` is a mounted view's own SQL,
/// so a result table would only copy what the columnar cache already serves.
pub fn is_plain_scan<'a>(
    request: &QueryRequest,
    sql: &str,
    views: impl IntoIterator<Item = &'a ViewInfo>,
) -> bool {
    request.filters.is_empty()
        && request.sorts.is_empty()
        && request.dedupe_columns.is_empty()
        && views.into_iter().any(|view| view.sql == sql)
}

#[cfg(test)]
mod tests;
