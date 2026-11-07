//! The background worker that imports flat files into the columnar cache, one at a time.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, bail};
use duckdb::{Connection, InterruptHandle};

use super::columnar_files::{evict, remove_partial, remove_stray_partials};
use super::columnar_key::{ColumnarJob, ColumnarKey};
use crate::runtime::dirs::{Dirs, configure_spill, lock_down_paths};

/// The most the columnar folder may hold before the least recently used files go.
// ponytail: fixed 10 GB cap; becomes a setting only if users need it.
const CACHE_CAP: u64 = 10 * 1024 * 1024 * 1024;

/// How often shutdown interrupts the import again until the worker thread ends.
const REINTERRUPT_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Default)]
struct Shared {
    is_stopping: AtomicBool,
    running: Mutex<Option<Arc<InterruptHandle>>>,
    #[cfg(test)]
    hold: Mutex<Option<Hold>>,
}

/// Test hook: where the import pauses between statements, and how a test lets it go on.
#[cfg(test)]
struct Hold {
    reached: Sender<()>,
    release: Receiver<()>,
}

impl Shared {
    fn running(&self) -> MutexGuard<'_, Option<Arc<InterruptHandle>>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn is_stopping(&self) -> bool {
        self.is_stopping.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    fn hold_before_create(&self) {
        let hold = self
            .hold
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(Hold { reached, release }) = hold {
            reached.send(()).ok();
            release.recv().ok();
        }
    }
}

/// Imports flat files into per-source DuckDB files, one at a time, on its own thread.
pub struct ColumnarWorker {
    sender: Option<Sender<ColumnarJob>>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl ColumnarWorker {
    /// Clears stray `*.partial` files, evicts down to the cap, then starts the worker thread.
    /// `on_ready` is called with the source id and cache key after each import lands.
    pub fn start(
        cache_dir: &Path,
        spill_dir: &Path,
        on_ready: Box<dyn Fn(String, ColumnarKey) + Send>,
    ) -> ColumnarWorker {
        Self::with_cap(CACHE_CAP, cache_dir, spill_dir, on_ready)
    }

    /// Like `start`, with the folder capped at `cap` bytes instead of 10 GB.
    pub fn with_cap(
        cap: u64,
        cache_dir: &Path,
        spill_dir: &Path,
        on_ready: Box<dyn Fn(String, ColumnarKey) + Send>,
    ) -> ColumnarWorker {
        remove_stray_partials(&cache_dir.join("columnar"));
        evict(&cache_dir.join("columnar"), cap);
        let (sender, receiver) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let context = WorkerContext {
            cache_dir: cache_dir.to_owned(),
            // `configure_spill` derives the spill folder as `<cache>/duckdb-tmp`.
            spill_dirs: Dirs {
                data: cache_dir.to_owned(),
                cache: spill_dir.parent().unwrap_or(spill_dir).to_owned(),
            },
            shared: Arc::clone(&shared),
            cap,
            on_ready,
        };
        let thread = thread::spawn(move || context.run(&receiver));
        ColumnarWorker {
            sender: Some(sender),
            shared,
            thread: Some(thread),
        }
    }

    pub fn enqueue(&self, job: ColumnarJob) {
        let Some(sender) = &self.sender else {
            tracing::warn!(source_id = %job.source_id, "columnar worker is stopped; job dropped");
            return;
        };
        if let Err(rejected) = sender.send(job) {
            tracing::warn!(source_id = %rejected.0.source_id, "columnar worker is gone; job dropped");
        }
    }

    /// Stops accepting jobs, interrupts the running import and waits for the thread.
    pub fn shutdown(&mut self) {
        self.shared.is_stopping.store(true, Ordering::SeqCst);
        self.sender = None;
        let Some(thread) = self.thread.take() else {
            return;
        };
        // DuckDB drops an interrupt that lands between statements, so keep interrupting until
        // the thread ends. The job also checks the flag before each statement.
        while !thread.is_finished() {
            if let Some(handle) = self.shared.running().as_ref() {
                handle.interrupt();
            }
            thread::sleep(REINTERRUPT_INTERVAL);
        }
        if thread.join().is_err() {
            tracing::warn!("columnar worker thread panicked");
        }
    }
}

struct WorkerContext {
    cache_dir: PathBuf,
    spill_dirs: Dirs,
    shared: Arc<Shared>,
    cap: u64,
    on_ready: Box<dyn Fn(String, ColumnarKey) + Send>,
}

impl WorkerContext {
    fn run(&self, receiver: &Receiver<ColumnarJob>) {
        let mut failed = HashSet::new();
        for job in receiver {
            if self.shared.is_stopping() {
                break;
            }
            if job.key.is_ready(&self.cache_dir) || failed.contains(&job.key) {
                continue;
            }
            let outcome = self.import(&job);
            *self.shared.running() = None;
            match outcome {
                Ok(()) => {
                    evict(&self.cache_dir.join("columnar"), self.cap);
                    (self.on_ready)(job.source_id, job.key);
                }
                Err(error) => {
                    remove_partial(&job.key.partial_path(&self.cache_dir));
                    if !self.shared.is_stopping() {
                        // Display shows only the outermost context, never DuckDB's cell values.
                        tracing::warn!(source_id = %job.source_id, %error, "columnar import failed");
                        failed.insert(job.key);
                    }
                }
            }
        }
    }

    fn ensure_running(&self) -> anyhow::Result<()> {
        if self.shared.is_stopping() {
            bail!("import cancelled");
        }
        Ok(())
    }

    fn import(&self, job: &ColumnarJob) -> anyhow::Result<()> {
        let partial = job.key.partial_path(&self.cache_dir);
        fs::create_dir_all(self.cache_dir.join("columnar"))
            .context("could not create the columnar folder")?;
        remove_partial(&partial);
        let conn = Connection::open(&partial).context("could not create the columnar file")?;
        *self.shared.running() = Some(conn.interrupt_handle());
        self.ensure_running()?;
        configure_spill(&conn, &self.spill_dirs)?;
        self.ensure_running()?;
        lock_down_paths(&conn, &[job.source_path.to_string_lossy().into_owned()])?;
        #[cfg(test)]
        self.shared.hold_before_create();
        self.ensure_running()?;
        conn.execute_batch(&format!(
            "CREATE TABLE data AS SELECT * FROM {}",
            job.scan_expression
        ))
        .context("could not import the source")?;
        conn.close()
            .map_err(|(_, error)| error)
            .context("could not close the columnar file")?;
        fs::rename(&partial, job.key.final_path(&self.cache_dir))
            .context("could not publish the columnar file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
