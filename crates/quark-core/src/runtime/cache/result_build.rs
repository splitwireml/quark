//! The background thread that builds a result table, and the entry bookkeeping it shares.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use duckdb::Connection;
use serde_json::Value;

use super::results::{Entries, MEMORY_DATABASE, RESULTS_SCHEMA, State, drop_statement, lock};
use crate::relation::params::bind;

/// How often a running build checks that its entry is still wanted.
const WATCH_INTERVAL: Duration = Duration::from_millis(10);

/// Moves a Building entry to `state`; false when it was superseded or evicted meanwhile.
pub(super) fn finish(entries: &Mutex<Entries>, key: &str, state: State) -> bool {
    let mut entries = lock(entries);
    let building = entries
        .iter_mut()
        .find(|(known, current)| known == key && *current == State::Building);
    building.map(|(_, current)| *current = state).is_some()
}

pub(super) fn is_building(entries: &Mutex<Entries>, key: &str) -> bool {
    lock(entries)
        .iter()
        .any(|(known, state)| known == key && *state == State::Building)
}

/// One build runs per engine: a newer trigger marks any running build Failed, so it is never retried
/// and its watcher interrupts it.
pub(super) fn supersede_builds(entries: &mut Entries) {
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
            &format!("CREATE TABLE {RESULTS_SCHEMA}.{table} AS {ordered_sql}"),
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

/// Builds `quark_mem.quark_results.<table>` from `ordered_sql` on a clone of `conn` and its own thread.
/// The entry becomes Ready when the table lands and Failed otherwise, so it is never retried.
pub(super) fn start(
    conn: &Connection,
    entries: &Arc<Mutex<Entries>>,
    key: &str,
    table: &str,
    ordered_sql: &str,
    params: Vec<Value>,
) -> Option<JoinHandle<()>> {
    let clone = conn
        .execute_batch(&format!(
            "ATTACH IF NOT EXISTS ':memory:' AS {MEMORY_DATABASE} (READ_WRITE);
             CREATE SCHEMA IF NOT EXISTS {RESULTS_SCHEMA}"
        ))
        .and_then(|()| conn.try_clone());
    let clone = match clone {
        Ok(clone) => clone,
        Err(error) => {
            tracing::debug!(table, %error, "result build could not start");
            finish(entries, key, State::Failed);
            return None;
        }
    };
    let entries = Arc::clone(entries);
    let (key, table, sql) = (key.to_owned(), table.to_owned(), ordered_sql.to_owned());
    Some(thread::spawn(move || {
        run_build(&clone, &entries, &key, &table, &sql, &params);
    }))
}
