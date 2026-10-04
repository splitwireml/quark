//! Request cancellation: a dropped request interrupts only the query it owns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::engine::{Engine, EngineInner};

#[derive(Debug, thiserror::Error)]
#[error("request cancelled")]
pub struct Cancelled;

/// The part of a [`CancelGuard`] a blocking task needs: it outlives the guard being dropped.
#[derive(Clone)]
pub struct Ticket {
    id: u64,
    is_cancelled: Arc<AtomicBool>,
}

/// Held by a request. Dropping it while armed cancels the request's ticket.
pub struct CancelGuard {
    engine: Arc<Engine>,
    ticket: Ticket,
    is_armed: bool,
}

fn lock_active(active: &Mutex<Option<u64>>) -> MutexGuard<'_, Option<u64>> {
    active.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Engine {
    pub fn guard(self: &Arc<Self>) -> CancelGuard {
        CancelGuard {
            engine: Arc::clone(self),
            ticket: Ticket {
                id: self.next_ticket.fetch_add(1, Ordering::Relaxed),
                is_cancelled: Arc::new(AtomicBool::new(false)),
            },
            is_armed: true,
        }
    }

    /// Runs `f` on the connection unless the ticket was cancelled while waiting for the lock.
    pub fn run_guarded<T>(
        &self,
        ticket: &Ticket,
        f: impl FnOnce(&mut EngineInner) -> T,
    ) -> Result<T, Cancelled> {
        let mut inner = self.lock();
        {
            // Check and claim under `active`, so a drop either sees the flag here or sees our id.
            let mut active = lock_active(&self.active);
            if ticket.is_cancelled.load(Ordering::SeqCst) {
                return Err(Cancelled);
            }
            *active = Some(ticket.id);
        }
        let result = f(&mut inner);
        *lock_active(&self.active) = None;
        Ok(result)
    }
}

impl Ticket {
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::SeqCst)
    }
}

impl CancelGuard {
    pub fn ticket(&self) -> Ticket {
        self.ticket.clone()
    }

    /// Turns the guard off: dropping it no longer cancels anything.
    pub fn disarm(&mut self) {
        self.is_armed = false;
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        if !self.is_armed {
            return;
        }
        self.ticket.is_cancelled.store(true, Ordering::SeqCst);
        let active = lock_active(&self.engine.active);
        if *active == Some(self.ticket.id) {
            self.engine.interrupt.interrupt();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineKind;
    use duckdb::Connection;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    const LONG_QUERY: &str = "SELECT sum(i) FROM range(10000000000) t(i)";
    const SHORT_QUERY: &str = "SELECT count(*) FROM range(50000000)";

    fn engine() -> Arc<Engine> {
        let kind = EngineKind::Node {
            node_id: "s1".to_owned(),
        };
        Arc::new(Engine::new(kind, 1, Connection::open_in_memory().unwrap()))
    }

    /// Runs short queries on the ticket until `release` is set; reports each query's success.
    fn spin_until(
        engine: &Arc<Engine>,
        ticket: Ticket,
        started: mpsc::Sender<()>,
        release: Arc<AtomicBool>,
    ) -> thread::JoinHandle<bool> {
        let engine = Arc::clone(engine);
        thread::spawn(move || {
            engine
                .run_guarded(&ticket, |inner| {
                    started.send(()).unwrap();
                    let mut is_ok = true;
                    while !release.load(Ordering::SeqCst) {
                        is_ok &= inner
                            .conn
                            .query_row(SHORT_QUERY, [], |row| row.get::<_, i64>(0))
                            .is_ok();
                    }
                    is_ok
                })
                .unwrap()
        })
    }

    #[test]
    fn dropping_guard_interrupts_running_query() {
        let engine = engine();
        let guard = engine.guard();
        let ticket = guard.ticket();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let runner = Arc::clone(&engine);
        thread::spawn(move || {
            let outcome = runner.run_guarded(&ticket, |inner| {
                started_tx.send(()).unwrap();
                inner
                    .conn
                    .query_row(LONG_QUERY, [], |row| row.get::<_, f64>(0))
            });
            done_tx.send(outcome).unwrap();
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(100));

        let dropped_at = Instant::now();
        drop(guard);
        let outcome = done_rx.recv_timeout(Duration::from_millis(500)).unwrap();

        assert!(dropped_at.elapsed() < Duration::from_millis(500));
        let error = outcome.unwrap().unwrap_err();
        assert!(error.to_string().contains("INTERRUPT"), "{error}");
    }

    #[test]
    fn cancelled_before_lock_skips_execution() {
        let engine = engine();
        let guard = engine.guard();
        let ticket = guard.ticket();
        let held = engine.lock();
        let runner = Arc::clone(&engine);
        let waiting = thread::spawn(move || {
            let mut has_run = false;
            let outcome = runner.run_guarded(&ticket, |_| has_run = true);
            (outcome.is_err(), has_run)
        });
        thread::sleep(Duration::from_millis(50));

        drop(guard);
        drop(held);

        assert_eq!(waiting.join().unwrap(), (true, false));
    }

    #[test]
    fn stale_interrupt_does_not_hit_the_next_query() {
        let engine = engine();
        let first = engine.guard();
        engine.run_guarded(&first.ticket(), |_| ()).unwrap();
        let second = engine.guard();
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let running = spin_until(&engine, second.ticket(), started_tx, Arc::clone(&release));
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));

        drop(first);
        thread::sleep(Duration::from_millis(100));
        release.store(true, Ordering::SeqCst);

        assert!(running.join().unwrap());
    }

    #[test]
    fn waiting_request_does_not_interrupt_running_one() {
        let engine = engine();
        let running_guard = engine.guard();
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let running = spin_until(
            &engine,
            running_guard.ticket(),
            started_tx,
            Arc::clone(&release),
        );
        started_rx.recv().unwrap();
        let waiting_guard = engine.guard();
        let waiting_ticket = waiting_guard.ticket();
        let waiter = Arc::clone(&engine);
        let waiting = thread::spawn(move || waiter.run_guarded(&waiting_ticket, |_| ()).is_err());
        thread::sleep(Duration::from_millis(50));

        drop(waiting_guard);
        thread::sleep(Duration::from_millis(100));
        release.store(true, Ordering::SeqCst);

        assert!(running.join().unwrap());
        assert!(waiting.join().unwrap());
        drop(running_guard);
    }

    #[test]
    fn disarmed_guard_does_not_cancel() {
        let engine = engine();
        let mut guard = engine.guard();
        let ticket = guard.ticket();
        guard.disarm();
        drop(guard);

        assert_eq!(engine.run_guarded(&ticket, |_| 7).unwrap(), 7);
    }
}
