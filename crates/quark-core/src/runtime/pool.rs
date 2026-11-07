//! A fixed pool of DuckDB reader connections cloned from an engine's primary connection.

use std::ops::Deref;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::available_parallelism;

use duckdb::{Connection, InterruptHandle};

/// How many readers an engine gets: half the cores, between 2 and 4.
pub fn reader_count() -> usize {
    reader_count_for(available_parallelism().map_or(1, usize::from))
}

pub(crate) fn reader_count_for(cores: usize) -> usize {
    (cores / 2).clamp(2, 4)
}

/// One pooled connection and the handle that interrupts its running query.
pub struct Reader {
    pub conn: Connection,
    interrupt: Arc<InterruptHandle>,
}

impl Reader {
    fn new(conn: Connection) -> Self {
        let interrupt = conn.interrupt_handle();
        Self { conn, interrupt }
    }

    pub fn interrupt_handle(&self) -> Arc<InterruptHandle> {
        Arc::clone(&self.interrupt)
    }
}

pub struct ReaderPool {
    idle: Mutex<Vec<Reader>>,
    freed: Condvar,
}

impl ReaderPool {
    /// Clones `size` readers from `primary`; they share its database, settings and lock-down.
    pub fn new(primary: &Connection, size: usize) -> duckdb::Result<Self> {
        let idle = (0..size)
            .map(|_| primary.try_clone().map(Reader::new))
            .collect::<duckdb::Result<Vec<_>>>()?;
        Ok(Self {
            idle: Mutex::new(idle),
            freed: Condvar::new(),
        })
    }

    /// Takes an idle reader, blocking until one is free.
    pub fn lease(&self) -> Lease<'_> {
        let mut idle = self.idle();
        let reader = loop {
            if let Some(reader) = idle.pop() {
                break reader;
            }
            idle = self
                .freed
                .wait(idle)
                .unwrap_or_else(PoisonError::into_inner);
        };
        Lease {
            pool: self,
            reader: Some(reader),
        }
    }

    fn idle(&self) -> MutexGuard<'_, Vec<Reader>> {
        self.idle.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A reader on loan; dropping it, even while unwinding, returns it to the pool.
pub struct Lease<'pool> {
    pool: &'pool ReaderPool,
    reader: Option<Reader>,
}

impl Deref for Lease<'_> {
    type Target = Reader;

    #[expect(
        clippy::expect_used,
        reason = "the reader leaves the lease only inside drop"
    )]
    fn deref(&self) -> &Reader {
        self.reader
            .as_ref()
            .expect("a lease holds its reader until it is dropped")
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if let Some(reader) = self.reader.take() {
            self.pool.idle().push(reader);
            self.pool.freed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
