//! Tests for the reader count, leasing, panic safety and what readers inherit from the primary.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::runtime::dirs::lock_down_paths;

/// Takes a few hundred milliseconds on one reader.
const SLOW_QUERY: &str =
    "SELECT count(*) FROM range(3000) a(x), range(100000) b(y) WHERE (x + y) % 7 = 0";

fn pool_of(size: usize) -> ReaderPool {
    let primary = Connection::open_in_memory().unwrap();
    ReaderPool::new(&primary, size).unwrap()
}

#[test]
fn reader_count_is_half_the_cores_within_bounds() {
    assert_eq!(reader_count_for(1), 2);
    assert_eq!(reader_count_for(4), 2);
    assert_eq!(reader_count_for(10), 4);
    assert_eq!(reader_count_for(64), 4);
    assert!((2..=4).contains(&reader_count()));
}

#[test]
fn two_leases_query_at_the_same_time() {
    let pool = pool_of(2);
    let leased = AtomicUsize::new(0);

    let spans: Vec<(Instant, Instant)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let lease = pool.lease();
                    leased.fetch_add(1, Ordering::SeqCst);
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while leased.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    let started = Instant::now();
                    let total: i64 = lease
                        .conn
                        .query_row(SLOW_QUERY, [], |row| row.get(0))
                        .unwrap();
                    assert!(total > 0);
                    (started, Instant::now())
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let last_start = spans.iter().map(|span| span.0).max().unwrap();
    let first_end = spans.iter().map(|span| span.1).min().unwrap();
    assert!(
        last_start < first_end,
        "both queries must start before either finishes"
    );
}

#[test]
fn lease_waits_while_every_reader_is_busy() {
    let pool = pool_of(2);
    let first = pool.lease();
    let second = pool.lease();
    let (sender, receiver) = mpsc::channel();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let _third = pool.lease();
            sender.send(()).unwrap();
        });
        assert!(receiver.recv_timeout(Duration::from_millis(200)).is_err());
        drop(first);
        assert!(receiver.recv_timeout(Duration::from_secs(10)).is_ok());
    });
    drop(second);
}

#[test]
fn lease_is_returned_when_the_holder_panics() {
    let pool = pool_of(1);

    let outcome = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _lease = pool.lease();
                panic!("holder fails while leasing");
            })
            .join()
    });

    assert!(outcome.is_err());
    let lease = pool.lease();
    let one: i64 = lease
        .conn
        .query_row("SELECT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(one, 1);
}

#[test]
fn readers_inherit_the_lock_down() {
    let root = tempfile::tempdir().unwrap();
    let allowed = root.path().join("allowed.csv");
    let other = root.path().join("other.csv");
    std::fs::write(&allowed, "a\n1\n2\n").unwrap();
    std::fs::write(&other, "a\n1\n").unwrap();
    let primary = Connection::open_in_memory().unwrap();
    lock_down_paths(&primary, &[allowed.to_string_lossy().into_owned()]).unwrap();
    let pool = ReaderPool::new(&primary, 2).unwrap();

    let reader = pool.lease();
    let count = |file: &std::path::Path| -> duckdb::Result<i64> {
        let path = file.to_string_lossy();
        reader.conn.query_row(
            &format!("SELECT count(*) FROM read_csv_auto('{path}')"),
            [],
            |row| row.get(0),
        )
    };

    assert_eq!(count(&allowed).unwrap(), 2);
    assert!(count(&other).is_err());
    assert!(
        reader
            .conn
            .execute_batch("SET enable_external_access = true")
            .is_err()
    );
}

#[test]
fn readers_see_views_created_later_on_the_primary() {
    let primary = Connection::open_in_memory().unwrap();
    let pool = ReaderPool::new(&primary, 2).unwrap();

    primary
        .execute_batch("CREATE VIEW later AS SELECT 42 AS answer")
        .unwrap();

    let reader = pool.lease();
    let answer: i64 = reader
        .conn
        .query_row("SELECT answer FROM later", [], |row| row.get(0))
        .unwrap();
    assert_eq!(answer, 42);
}

#[test]
fn a_reader_exposes_an_interrupt_handle() {
    let pool = pool_of(2);
    let lease = pool.lease();

    lease.interrupt_handle().interrupt();

    let one: i64 = lease
        .conn
        .query_row("SELECT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(one, 1);
}
