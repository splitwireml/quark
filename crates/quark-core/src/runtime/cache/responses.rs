//! Per-engine LRU of analysis responses, valid only while the engine's source files are unchanged.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::time::Instant;

use duckdb::Connection;
use serde_json::Value;

use crate::error::ApiResult;
use crate::relation::filtered::Relation;
use crate::runtime::engine::{Engine, lock_recovering};
use crate::runtime::mount::{database_paths, is_database};
use crate::runtime::registry::SourceRecord;

const CAPACITY: usize = 128;

/// Words whose presence makes SQL depend on more than the source files.
const UNSAFE_WORDS: [&str; 6] = [
    "current_timestamp",
    "current_time",
    "localtimestamp",
    "localtime",
    "tablesample",
    "sample",
];

/// File extensions that suggest a nested scan of a file the engine does not track.
const SCAN_EXTENSIONS: [&str; 9] = [
    "csv", "tsv", "parquet", "json", "jsonl", "ndjson", "xlsx", "duckdb", "db",
];

/// Functions and macros whose result is not a pure function of the data.
const VOLATILE_NAMES: &str = "SELECT DISTINCT function_name FROM duckdb_functions() \
    WHERE stability != 'CONSISTENT' \
    OR (function_type IN ('table', 'macro', 'table_macro') AND function_name != 'query')";

const STORED_VIEWS: &str = "SELECT count(*) FROM duckdb_views() v \
    JOIN duckdb_databases() d USING (database_name) WHERE NOT v.internal AND d.path IS NOT NULL";

/// What identifies one version of a file: any edit changes at least one field.
#[derive(Debug, PartialEq, Eq)]
pub struct Fingerprint {
    size: u64,
    modified_ns: i128,
    inode: u64,
    changed_ns: i128,
}

/// The file's fingerprint, or `None` when it cannot be read.
pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = fs::metadata(path).ok()?;
    let (modified_ns, inode, changed_ns) = times(&meta)?;
    Some(Fingerprint {
        size: meta.len(),
        modified_ns,
        inode,
        changed_ns,
    })
}

#[cfg(unix)]
fn times(meta: &fs::Metadata) -> Option<(i128, u64, i128)> {
    use std::os::unix::fs::MetadataExt;
    let nanos = |secs: i64, extra: i64| i128::from(secs) * 1_000_000_000 + i128::from(extra);
    Some((
        nanos(meta.mtime(), meta.mtime_nsec()),
        meta.ino(),
        nanos(meta.ctime(), meta.ctime_nsec()),
    ))
}

#[cfg(not(unix))]
fn times(meta: &fs::Metadata) -> Option<(i128, u64, i128)> {
    let since_epoch = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some((i128::try_from(since_epoch.as_nanos()).ok()?, 0, 0))
}

/// The files an engine over `sources` reads: each source, plus the write-ahead log of each database.
pub(crate) fn source_files(sources: &[SourceRecord]) -> Vec<PathBuf> {
    let mut files = Vec::with_capacity(sources.len());
    for source in sources {
        files.push(source.source.clone());
        if is_database(&source.source) {
            let [log, _] = database_paths(&source.source);
            files.push(PathBuf::from(log));
        }
    }
    files
}

#[derive(Debug, Default)]
pub struct ResponseCache {
    entries: HashMap<String, Value>,
    /// Keys from least to most recently used.
    order: VecDeque<String>,
}

impl ResponseCache {
    fn get(&mut self, key: &str) -> Option<Value> {
        let value = self.entries.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    fn insert(&mut self, key: String, value: Value) {
        if self.entries.insert(key.clone(), value).is_some() {
            self.touch(&key);
            return;
        }
        self.order.push_back(key);
        if self.entries.len() > CAPACITY
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
    }

    fn touch(&mut self, key: &str) {
        // ponytail: O(n) scan of the order queue; fine at 128 entries, a linked map is the upgrade path.
        let moved = self
            .order
            .iter()
            .position(|queued| queued == key)
            .and_then(|index| self.order.remove(index));
        if let Some(key) = moved {
            self.order.push_back(key);
        }
    }
}

/// Lowercase identifier words: runs of word characters that start with a letter or underscore.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .filter(|word| word.starts_with(|ch: char| ch.is_ascii_lowercase() || ch == '_'))
}

/// Whether `params_json` mentions `query` or a file name that a nested scan could read.
fn has_indirect_source(params_json: &str) -> bool {
    let lowered = params_json.to_lowercase();
    words(&lowered).any(|word| word == "query")
        || lowered.split('.').skip(1).any(|after_dot| {
            let end = after_dot
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .unwrap_or(after_dot.len());
            SCAN_EXTENSIONS.contains(&&after_dot[..end])
        })
}

impl Engine {
    /// The response cache; hold the guard only to read or write one entry.
    fn responses(&self) -> MutexGuard<'_, ResponseCache> {
        lock_recovering(&self.responses)
    }

    /// Names of functions and macros that make a response uncacheable, computed once per engine.
    fn volatile_names(&self, conn: &Connection) -> duckdb::Result<&HashSet<String>> {
        if let Some(names) = self.volatile.get() {
            return Ok(names);
        }
        let mut statement = conn.prepare(VOLATILE_NAMES)?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|name| name.map(|name| name.to_lowercase()))
            .collect::<duckdb::Result<HashSet<_>>>()?;
        Ok(self.volatile.get_or_init(|| names))
    }

    /// Whether a response over this relation can only change when a source file does.
    pub fn is_cacheable(
        &self,
        conn: &Connection,
        relation_sql: &str,
        params_json: &str,
    ) -> duckdb::Result<bool> {
        if has_indirect_source(params_json) {
            return Ok(false);
        }
        let stored_views: i64 = conn.query_row(STORED_VIEWS, [], |row| row.get(0))?;
        if stored_views > 0 {
            return Ok(false);
        }
        let volatile = self.volatile_names(conn)?;
        let text = format!("{relation_sql} {params_json}").to_lowercase();
        Ok(!words(&text).any(|word| UNSAFE_WORDS.contains(&word) || volatile.contains(word)))
    }

    /// The fingerprint of every source file, or `None` when one of them has gone missing.
    /// A write-ahead log that does not exist is normal and fingerprints as absent.
    fn source_fingerprints(&self) -> Option<Vec<Option<Fingerprint>>> {
        self.source_files
            .iter()
            .map(|path| match fingerprint(path) {
                None if path.extension().is_some_and(|ext| ext == "wal") => Some(None),
                print => print.map(Some),
            })
            .collect()
    }

    /// Returns the cached response for this request, or runs `compute` and keeps its answer.
    /// `detail` carries what the relation does not: the column, the chart spec. Errors are not cached.
    pub fn cached_response(
        &self,
        conn: &Connection,
        kind: &str,
        relation: &Relation,
        detail: &Value,
        compute: impl FnOnce() -> ApiResult<Value>,
    ) -> ApiResult<Value> {
        let started = Instant::now();
        let params_json = Value::Array(relation.params.clone()).to_string();
        let cacheable = self
            .is_cacheable(conn, &relation.sql, &params_json)
            .unwrap_or(false);
        let prints = self.source_fingerprints().filter(|_| cacheable);
        let Some(prints) = prints else {
            return compute();
        };
        let key = format!(
            "{kind}\0{}\0{params_json}\0{detail}\0{prints:?}",
            relation.sql
        );
        let hit = self.responses().get(&key);
        if let Some(mut response) = hit {
            if let Some(elapsed) = response.get_mut("elapsed_ms") {
                let millis = started.elapsed().as_secs_f64() * 1000.0;
                *elapsed = Value::from((millis * 1000.0).round() / 1000.0);
            }
            return Ok(response);
        }
        let response = compute()?;
        self.responses().insert(key, response.clone());
        Ok(response)
    }
}

#[cfg(test)]
mod tests;
