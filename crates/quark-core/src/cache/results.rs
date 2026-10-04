//! Per-engine bookkeeping of repeated queries: the second identical request builds a result table.
//!
//! Keys are built with `stats_key(ordered_sql, params)`: the ordered SQL plus the JSON of its parameters.

use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use duckdb::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::mount::ViewInfo;
use crate::page::bind;
use crate::query::QueryRequest;

const CAPACITY: usize = 4;
/// How often a running build checks that its entry is still wanted.
const WATCH_INTERVAL: Duration = Duration::from_millis(10);

/// What to do with a request for a query the cache may have seen before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultAction {
    ServeDirect,
    StartBuild(String),
    ServeFrom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Seen,
    Building,
    Ready,
    Failed,
}

type Entries = Vec<(String, State)>;

pub struct ResultCache {
    capacity: usize,
    /// Keys with their state, from least to most recently used. The build thread updates it too.
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
    format!("DROP TABLE IF EXISTS quark_results.{table}")
}

fn lock(entries: &Mutex<Entries>) -> MutexGuard<'_, Entries> {
    entries.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Moves a Building entry to `state`; false when it was superseded or evicted meanwhile.
fn finish(entries: &Mutex<Entries>, key: &str, state: State) -> bool {
    let mut entries = lock(entries);
    let building = entries
        .iter_mut()
        .find(|(known, current)| known == key && *current == State::Building);
    building.map(|(_, current)| *current = state).is_some()
}

fn is_building(entries: &Mutex<Entries>, key: &str) -> bool {
    lock(entries)
        .iter()
        .any(|(known, state)| known == key && *state == State::Building)
}

/// One build runs per engine: a newer trigger marks any running build Failed, so it is never retried
/// and its watcher interrupts it.
fn supersede_builds(entries: &mut Entries) {
    for (_, state) in entries {
        if *state == State::Building {
            *state = State::Failed;
        }
    }
}

fn run_build(
    conn: &Connection,
    entries: &Mutex<Entries>,
    key: &str,
    table: &str,
    ordered_sql: &str,
    params: &[Value],
) {
    let interrupt = conn.interrupt_handle();
    let is_done = AtomicBool::new(false);
    let created = thread::scope(|scope| {
        // An interrupt sent before the statement starts is lost, so keep checking until it ends.
        scope.spawn(|| {
            while !is_done.load(Ordering::SeqCst) {
                if !is_building(entries, key) {
                    interrupt.interrupt();
                }
                thread::sleep(WATCH_INTERVAL);
            }
        });
        let created = conn.execute(
            &format!("CREATE TABLE quark_results.{table} AS {ordered_sql}"),
            bind(params),
        );
        is_done.store(true, Ordering::SeqCst);
        created
    });
    // DuckDB's error text can quote cell values, so only the table is logged.
    if created.is_err() {
        tracing::debug!(table, "result build failed or was interrupted");
    }
    let state = if created.is_ok() {
        State::Ready
    } else {
        State::Failed
    };
    if !finish(entries, key, state)
        && created.is_ok()
        && let Err(error) = conn.execute_batch(&drop_statement(table))
    {
        tracing::warn!(table, %error, "could not drop an unused result table");
    }
}

impl ResultCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
            dropped: Vec::new(),
            build: None,
        }
    }

    pub fn note_request(&mut self, key: &str) -> ResultAction {
        let shared = Arc::clone(&self.entries);
        let mut entries = lock(&shared);
        let Some(index) = entries.iter().position(|(known, _)| known == key) else {
            self.insert(&mut entries, key);
            return ResultAction::ServeDirect;
        };
        let (key, state) = entries.remove(index);
        let (state, action) = match state {
            State::Seen => {
                supersede_builds(&mut entries);
                let table = table_name(&key);
                (State::Building, ResultAction::StartBuild(table))
            }
            State::Ready => (State::Ready, ResultAction::ServeFrom(table_name(&key))),
            state => (state, ResultAction::ServeDirect),
        };
        entries.push((key, state));
        action
    }

    pub fn mark_ready(&mut self, key: &str) {
        finish(&self.entries, key, State::Ready);
    }

    pub fn mark_failed(&mut self, key: &str) {
        finish(&self.entries, key, State::Failed);
    }

    /// Builds `quark_results.<table>` from `ordered_sql` on a clone of `conn` and its own thread.
    /// The entry becomes Ready when the table lands and Failed otherwise, so it is never retried.
    pub fn start_build(
        &mut self,
        conn: &Connection,
        key: &str,
        table: &str,
        ordered_sql: &str,
        params: Vec<Value>,
    ) {
        let clone = conn
            .execute_batch("CREATE SCHEMA IF NOT EXISTS quark_results")
            .and_then(|()| conn.try_clone());
        let clone = match clone {
            Ok(clone) => clone,
            Err(error) => {
                tracing::debug!(table, %error, "result build could not start");
                self.mark_failed(key);
                return;
            }
        };
        let entries = Arc::clone(&self.entries);
        let (key, table, sql) = (key.to_owned(), table.to_owned(), ordered_sql.to_owned());
        self.build = Some(thread::spawn(move || {
            run_build(&clone, &entries, &key, &table, &sql, &params);
        }));
    }

    /// Tables of evicted entries since the last call; run [`drop_statement`] for each.
    pub fn take_dropped(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dropped)
    }

    fn insert(&mut self, entries: &mut Entries, key: &str) {
        if entries.len() >= self.capacity {
            let (oldest, state) = entries.remove(0);
            if matches!(state, State::Building | State::Ready) {
                self.dropped.push(table_name(&oldest));
            }
        }
        entries.push((key.to_owned(), State::Seen));
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
mod tests {
    use super::*;
    use crate::cache::stats::stats_key;
    use crate::query::{Connector, Filter};
    use serde_json::json;
    use std::time::{Duration, Instant};

    /// Runs for far longer than any test, so it only ends when interrupted.
    const LONG_BUILD: &str = "SELECT sum(i) AS total FROM range(1000000000000) t(i)";

    fn key(sql: &str) -> String {
        stats_key(sql, &[json!(1)])
    }

    fn view(sql: &str) -> ViewInfo {
        ViewInfo {
            id: "v".to_owned(),
            project_id: "p".to_owned(),
            source_id: "s".to_owned(),
            source_name: "sales".to_owned(),
            node_id: "n".to_owned(),
            name: "sales".to_owned(),
            schema: "main".to_owned(),
            kind: "view".to_owned(),
            columns: vec!["a".to_owned()],
            sql: sql.to_owned(),
        }
    }

    #[test]
    fn first_request_serves_direct_second_starts_build() {
        let mut cache = ResultCache::default();
        let k = key("SELECT 1");

        assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
        let ResultAction::StartBuild(table) = cache.note_request(&k) else {
            panic!("second request should start a build");
        };
        assert_eq!(table, table_name(&k));
        assert_eq!(table.len(), 18);
        assert!(table.starts_with("r_"));
        assert!(table[2..].bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
    }

    #[test]
    fn only_one_build_is_in_flight() {
        let mut cache = ResultCache::default();
        let (a, b) = (key("SELECT a"), key("SELECT b"));
        cache.note_request(&a);
        cache.note_request(&b);

        assert!(matches!(
            cache.note_request(&a),
            ResultAction::StartBuild(_)
        ));
        assert!(matches!(
            cache.note_request(&b),
            ResultAction::StartBuild(_)
        ));

        assert_eq!(cache.note_request(&a), ResultAction::ServeDirect);
        assert_eq!(cache.note_request(&b), ResultAction::ServeDirect);
    }

    #[test]
    fn ready_entries_serve_from_table() {
        let mut cache = ResultCache::default();
        let k = key("SELECT 1");
        cache.note_request(&k);
        cache.note_request(&k);

        cache.mark_ready(&k);

        assert_eq!(
            cache.note_request(&k),
            ResultAction::ServeFrom(table_name(&k))
        );
    }

    #[test]
    fn capacity_evicts_and_returns_dropped_table() {
        let mut cache = ResultCache::with_capacity(2);
        let (a, b, c, d) = (key("a"), key("b"), key("c"), key("d"));
        cache.note_request(&a);
        cache.note_request(&a);
        cache.mark_ready(&a);
        cache.note_request(&b);

        cache.note_request(&c);
        assert_eq!(cache.take_dropped(), vec![table_name(&a)]);

        cache.note_request(&d);
        assert!(cache.take_dropped().is_empty());
        assert_eq!(
            drop_statement(&table_name(&a)),
            format!("DROP TABLE IF EXISTS quark_results.{}", table_name(&a))
        );
    }

    #[test]
    fn plain_scans_are_skipped() {
        let views = [view("SELECT * FROM sales")];
        let plain = QueryRequest::default();
        let filtered = QueryRequest {
            filters: vec![Filter {
                column: "a".to_owned(),
                operator: "=".to_owned(),
                value: Some(json!(1)),
                connector: Connector::default(),
            }],
            ..QueryRequest::default()
        };

        assert!(is_plain_scan(&plain, "SELECT * FROM sales", &views));
        assert!(!is_plain_scan(&plain, "SELECT 2", &views));
        assert!(!is_plain_scan(&filtered, "SELECT * FROM sales", &views));
    }

    fn wait_until(mut is_done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !is_done() {
            assert!(Instant::now() < deadline, "timed out waiting");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn state_of(cache: &ResultCache, key: &str) -> Option<State> {
        lock(&cache.entries)
            .iter()
            .find(|(known, _)| known == key)
            .map(|(_, state)| *state)
    }

    fn result_tables(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT count(*) FROM duckdb_tables() WHERE schema_name = 'quark_results'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    /// Two requests for `key`: the second one's table name.
    fn trigger(cache: &mut ResultCache, key: &str) -> String {
        cache.note_request(key);
        let ResultAction::StartBuild(table) = cache.note_request(key) else {
            panic!("second request should start a build");
        };
        table
    }

    #[test]
    fn second_request_builds_while_pages_stay_direct() {
        let conn = Connection::open_in_memory().unwrap();
        let mut cache = ResultCache::default();
        let sql = "SELECT i, hash(i) AS h FROM range(3000000) t(i) WHERE i < ? ORDER BY h";
        let params = vec![json!(2_999_990)];
        let k = stats_key(sql, &params);
        let table = trigger(&mut cache, &k);

        cache.start_build(&conn, &k, &table, sql, params);

        assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
        wait_until(|| cache.note_request(&k) == ResultAction::ServeFrom(table.clone()));
        let rows: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM quark_results.{table}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 2_999_990);
    }

    #[test]
    fn superseded_build_is_interrupted() {
        let conn = Connection::open_in_memory().unwrap();
        let mut cache = ResultCache::default();
        let (a, b) = (key("a"), key("b"));
        cache.note_request(&b);
        let table_a = trigger(&mut cache, &a);
        cache.start_build(&conn, &a, &table_a, LONG_BUILD, Vec::new());
        let ResultAction::StartBuild(table_b) = cache.note_request(&b) else {
            panic!("second request should start a build");
        };
        cache.start_build(&conn, &b, &table_b, LONG_BUILD, Vec::new());

        assert_eq!(state_of(&cache, &a), Some(State::Failed));
        // The cache and build `b` hold the entries; build `a` lets go once interrupted.
        wait_until(|| Arc::strong_count(&cache.entries) == 2);
        assert_eq!(state_of(&cache, &b), Some(State::Building));
        assert_eq!(result_tables(&conn), 0);
    }

    #[test]
    fn dropping_the_cache_interrupts_a_running_build() {
        let conn = Connection::open_in_memory().unwrap();
        let mut cache = ResultCache::default();
        let k = key("long");
        let table = trigger(&mut cache, &k);
        cache.start_build(&conn, &k, &table, LONG_BUILD, Vec::new());
        let entries = Arc::clone(&cache.entries);

        drop(cache);

        assert_eq!(Arc::strong_count(&entries), 1);
    }

    #[test]
    fn failed_build_is_not_retried() {
        let conn = Connection::open_in_memory().unwrap();
        let mut cache = ResultCache::default();
        let k = key("missing");
        let table = trigger(&mut cache, &k);

        cache.start_build(&conn, &k, &table, "SELECT * FROM missing_table", Vec::new());

        wait_until(|| state_of(&cache, &k) == Some(State::Failed));
        for _ in 0..3 {
            assert_eq!(cache.note_request(&k), ResultAction::ServeDirect);
        }
        assert_eq!(result_tables(&conn), 0);
    }
}
