//! Request cancellation: a dropped request interrupts only the query it owns.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use duckdb::{Connection, InterruptHandle};

use crate::runtime::engine::{Engine, lock_recovering};

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

type Active = HashMap<u64, Arc<InterruptHandle>>;

fn lock_active(active: &Mutex<Active>) -> MutexGuard<'_, Active> {
    lock_recovering(active)
}

/// A ticket registered on a reader; dropping it, even while unwinding, ends the registration.
struct Registration<'a> {
    active: &'a Mutex<Active>,
    id: u64,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        lock_active(self.active).remove(&self.id);
    }
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

    /// Runs `work` on a pooled reader, unless the ticket was cancelled while waiting for one.
    /// A ticket stays registered on its reader while `work` runs, so only its own query is interrupted.
    pub fn read<T>(
        &self,
        ticket: Option<&Ticket>,
        work: impl FnOnce(&Connection) -> T,
    ) -> Result<T, Cancelled> {
        let lease = self.pool.lease();
        let _registration = match ticket {
            Some(ticket) => {
                // Check and register under `active`, so a drop either sees the flag here or sees our id.
                let mut active = lock_active(&self.active);
                if ticket.is_cancelled() {
                    return Err(Cancelled);
                }
                active.insert(ticket.id, lease.interrupt_handle());
                Some(Registration {
                    active: &self.active,
                    id: ticket.id,
                })
            }
            None => None,
        };
        Ok(work(&lease.conn))
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
        let id = self.ticket.id;
        {
            let active = lock_active(&self.engine.active);
            let Some(handle) = active.get(&id) else {
                return;
            };
            handle.interrupt();
        }
        // DuckDB drops an interrupt that lands between statements, so keep interrupting until
        // `read` unregisters the ticket. Interrupting under `active` means a reader that already
        // moved on to another query is never hit.
        let engine = Arc::clone(&self.engine);
        let spawned = thread::Builder::new()
            .name("quark-interrupt".to_owned())
            .spawn(move || {
                loop {
                    thread::sleep(REINTERRUPT_INTERVAL);
                    let active = lock_active(&engine.active);
                    let Some(handle) = active.get(&id) else {
                        break;
                    };
                    handle.interrupt();
                }
            });
        if spawned.is_err() {
            tracing::warn!("could not start the interrupt thread; the query gets one interrupt");
        }
    }
}

#[cfg(test)]
mod tests;
