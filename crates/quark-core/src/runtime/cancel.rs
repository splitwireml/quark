//! Request cancellation: a dropped request interrupts only the query it owns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::runtime::engine::{Engine, EngineInner};

/// How often a cancelled query is interrupted again until it stops.
const REINTERRUPT_INTERVAL: Duration = Duration::from_millis(5);

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
        {
            let active = lock_active(&self.engine.active);
            if *active != Some(self.ticket.id) {
                return;
            }
            self.engine.interrupt.interrupt();
        }
        // DuckDB drops an interrupt that lands between statements, so keep interrupting until
        // `run_guarded` clears `active`. Ids are unique, so this never hits another query.
        let engine = Arc::clone(&self.engine);
        let id = self.ticket.id;
        let spawned = thread::Builder::new()
            .name("quark-interrupt".to_owned())
            .spawn(move || {
                loop {
                    thread::sleep(REINTERRUPT_INTERVAL);
                    let active = lock_active(&engine.active);
                    if *active != Some(id) {
                        break;
                    }
                    engine.interrupt.interrupt();
                }
            });
        if spawned.is_err() {
            tracing::warn!("could not start the interrupt thread; the query gets one interrupt");
        }
    }
}

#[cfg(test)]
mod tests;
