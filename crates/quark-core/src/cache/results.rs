//! Per-engine bookkeeping of repeated queries: the second identical request builds a result table.
//!
//! Keys are built with `stats_key(ordered_sql, params)`: the ordered SQL plus the JSON of its parameters.

use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::mount::ViewInfo;
use crate::query::QueryRequest;

const CAPACITY: usize = 4;

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

#[derive(Debug)]
pub struct ResultCache {
    capacity: usize,
    /// Keys with their state, from least to most recently used.
    entries: Vec<(String, State)>,
    dropped: Vec<String>,
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

impl ResultCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Vec::with_capacity(capacity),
            dropped: Vec::new(),
        }
    }

    pub fn note_request(&mut self, key: &str) -> ResultAction {
        let Some(index) = self.entries.iter().position(|(known, _)| known == key) else {
            self.insert(key);
            return ResultAction::ServeDirect;
        };
        let (key, state) = self.entries.remove(index);
        let (state, action) = match state {
            State::Seen => {
                self.supersede_builds();
                let table = table_name(&key);
                (State::Building, ResultAction::StartBuild(table))
            }
            State::Ready => (State::Ready, ResultAction::ServeFrom(table_name(&key))),
            state => (state, ResultAction::ServeDirect),
        };
        self.entries.push((key, state));
        action
    }

    pub fn mark_ready(&mut self, key: &str) {
        self.set_state(key, State::Ready);
    }

    pub fn mark_failed(&mut self, key: &str) {
        self.set_state(key, State::Failed);
    }

    /// Tables of evicted entries since the last call; run [`drop_statement`] for each.
    pub fn take_dropped(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dropped)
    }

    fn insert(&mut self, key: &str) {
        if self.entries.len() >= self.capacity {
            let (oldest, state) = self.entries.remove(0);
            if matches!(state, State::Building | State::Ready) {
                self.dropped.push(table_name(&oldest));
            }
        }
        self.entries.push((key.to_owned(), State::Seen));
    }

    /// One build runs per engine: a newer trigger marks any running build Failed so it is never retried.
    fn supersede_builds(&mut self) {
        for (_, state) in &mut self.entries {
            if *state == State::Building {
                *state = State::Failed;
            }
        }
    }

    fn set_state(&mut self, key: &str, state: State) {
        if let Some((_, known)) = self.entries.iter_mut().find(|(known, _)| known == key) {
            *known = state;
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
}
