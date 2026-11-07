//! Tests for the page stats cache.

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
