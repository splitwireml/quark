//! Per-engine LRU of counts and null fractions, keyed by the filtered relation.

use std::collections::{HashMap, VecDeque};

use serde_json::Value;

use crate::page::ColumnSummary;

const CAPACITY: usize = 256;

/// What a page needs besides its rows: the column summaries and the row count.
#[derive(Debug, Clone, PartialEq)]
pub struct StatsEntry {
    pub columns: Vec<ColumnSummary>,
    pub total_rows: u64,
}

/// The relation SQL plus the compact JSON of its parameters.
/// The engine's generation is implicit: each engine owns its own cache.
pub fn stats_key(relation: &str, params: &[Value]) -> String {
    format!("{relation}\0{}", Value::Array(params.to_vec()))
}

#[derive(Debug)]
pub struct StatsCache {
    capacity: usize,
    entries: HashMap<String, StatsEntry>,
    /// Keys from least to most recently used.
    order: VecDeque<String>,
}

impl Default for StatsCache {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl StatsCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<StatsEntry> {
        let entry = self.entries.get(key)?.clone();
        self.touch(key);
        Some(entry)
    }

    pub fn insert(&mut self, key: String, entry: StatsEntry) {
        if self.entries.insert(key.clone(), entry).is_some() {
            self.touch(&key);
            return;
        }
        self.order.push_back(key);
        if self.entries.len() > self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
    }

    fn touch(&mut self, key: &str) {
        // ponytail: O(n) scan of the order queue; fine at 256 entries, a linked map is the upgrade path.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(total_rows: u64) -> StatsEntry {
        StatsEntry {
            columns: Vec::new(),
            total_rows,
        }
    }

    #[test]
    fn capacity_evicts_least_recent() {
        let mut cache = StatsCache::with_capacity(2);
        cache.insert("a".to_owned(), entry(1));
        cache.insert("b".to_owned(), entry(2));
        assert_eq!(cache.get("a"), Some(entry(1)));

        cache.insert("c".to_owned(), entry(3));

        assert_eq!(cache.get("b"), None);
        assert_eq!(cache.get("a"), Some(entry(1)));
        assert_eq!(cache.get("c"), Some(entry(3)));
    }

    #[test]
    fn reinserting_a_key_replaces_it_without_growing() {
        let mut cache = StatsCache::with_capacity(2);
        cache.insert("a".to_owned(), entry(1));
        cache.insert("b".to_owned(), entry(2));
        cache.insert("a".to_owned(), entry(9));

        cache.insert("c".to_owned(), entry(3));

        assert_eq!(cache.get("a"), Some(entry(9)));
        assert_eq!(cache.get("b"), None);
    }

    #[test]
    fn key_separates_relations_and_parameters() {
        let relation = "(SELECT * FROM t WHERE a > ?)";

        assert_eq!(
            stats_key(relation, &[json!(1)]),
            stats_key(relation, &[json!(1)])
        );
        assert_ne!(
            stats_key(relation, &[json!(1)]),
            stats_key(relation, &[json!(2)])
        );
        assert_ne!(stats_key(relation, &[]), stats_key("(SELECT 1)", &[]));
    }
}
