//! Columnar cache identity (a hash of the source file's identity and how it is scanned)
//! and the background worker that fills the cache.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt::Write;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use duckdb::{Connection, InterruptHandle};
use sha2::{Digest, Sha256};

use crate::engine::{Dirs, configure_spill, lock_down_paths};

/// The most the columnar folder may hold before the least recently used files go.
const CACHE_CAP: u64 = 10 * 1024 * 1024 * 1024;

/// Content-addressed name of a columnar cache file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnarKey(String);

impl ColumnarKey {
    pub fn new(path: &Path, scan_expression: &str, engine_version: &str) -> io::Result<Self> {
        let canonical = path.canonicalize()?;
        let metadata = canonical.metadata()?;
        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_os_str().as_encoded_bytes());
        for part in [
            metadata.len().to_string(),
            mtime.to_string(),
            engine_version.to_owned(),
            scan_expression.to_owned(),
        ] {
            hasher.update(b"\0");
            hasher.update(part.as_bytes());
        }
        let mut hex = String::with_capacity(64);
        for byte in hasher.finalize() {
            write!(hex, "{byte:02x}").map_err(io::Error::other)?;
        }
        Ok(Self(hex))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn final_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join("columnar")
            .join(format!("{}.duckdb", self.0))
    }

    pub fn partial_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join("columnar")
            .join(format!("{}.duckdb.partial", self.0))
    }

    pub fn is_ready(&self, cache_dir: &Path) -> bool {
        self.final_path(cache_dir).is_file()
    }

    /// Marks the cache file as just used, so eviction keeps it longest.
    pub fn touch(&self, cache_dir: &Path) {
        let mut options = File::options();
        #[cfg(windows)]
        {
            // Attribute access alone cannot clash with the handle DuckDB holds on the file.
            use std::os::windows::fs::OpenOptionsExt;
            options.access_mode(0x0100); // FILE_WRITE_ATTRIBUTES
        }
        #[cfg(not(windows))]
        options.write(true);
        if let Err(error) = options
            .open(self.final_path(cache_dir))
            .and_then(|file| file.set_modified(SystemTime::now()))
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::debug!(%error, "could not mark a columnar file as used");
        }
    }
}

/// One flat file to import into the columnar cache.
pub struct ColumnarJob {
    pub source_id: String,
    pub key: ColumnarKey,
    pub source_path: PathBuf,
    pub scan_expression: String,
}

#[derive(Default)]
struct Shared {
    is_stopping: AtomicBool,
    running: Mutex<Option<Arc<InterruptHandle>>>,
}

impl Shared {
    fn running(&self) -> MutexGuard<'_, Option<Arc<InterruptHandle>>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn is_stopping(&self) -> bool {
        self.is_stopping.load(Ordering::SeqCst)
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
    /// `on_ready` is called with the source id after each import lands.
    pub fn start(
        cache_dir: &Path,
        spill_dir: &Path,
        on_ready: Box<dyn Fn(String) + Send>,
    ) -> ColumnarWorker {
        Self::with_cap(CACHE_CAP, cache_dir, spill_dir, on_ready)
    }

    /// Like `start`, with the folder capped at `cap` bytes instead of 10 GB.
    pub fn with_cap(
        cap: u64,
        cache_dir: &Path,
        spill_dir: &Path,
        on_ready: Box<dyn Fn(String) + Send>,
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
        if let Some(handle) = self.shared.running().as_ref() {
            handle.interrupt();
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("columnar worker thread panicked");
        }
    }
}

struct WorkerContext {
    cache_dir: PathBuf,
    spill_dirs: Dirs,
    shared: Arc<Shared>,
    cap: u64,
    on_ready: Box<dyn Fn(String) + Send>,
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
                    (self.on_ready)(job.source_id);
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

    fn import(&self, job: &ColumnarJob) -> anyhow::Result<()> {
        let partial = job.key.partial_path(&self.cache_dir);
        fs::create_dir_all(self.cache_dir.join("columnar"))
            .context("could not create the columnar folder")?;
        remove_partial(&partial);
        let conn = Connection::open(&partial).context("could not create the columnar file")?;
        *self.shared.running() = Some(conn.interrupt_handle());
        if self.shared.is_stopping() {
            bail!("import cancelled");
        }
        configure_spill(&conn, &self.spill_dirs)?;
        lock_down_paths(&conn, &[job.source_path.to_string_lossy().into_owned()])?;
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

/// Deletes a partial import and the write-ahead log DuckDB may have left beside it.
fn remove_partial(partial: &Path) {
    let mut wal = OsString::from(partial);
    wal.push(".wal");
    for path in [partial, Path::new(&wal)] {
        if let Err(error) = fs::remove_file(path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not delete a partial columnar file");
        }
    }
}

/// Deletes the least recently used cache files until the folder fits in `cap` bytes.
/// The newest file stays even when it alone is over the cap: deleting it would make
/// the next mount import it again. A file the system refuses to delete (Windows keeps
/// an attached one) waits for the next pass.
fn evict(columnar_dir: &Path, cap: u64) {
    let Ok(entries) = fs::read_dir(columnar_dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "duckdb"))
        .filter_map(|path| {
            let metadata = path.metadata().ok()?;
            Some((metadata.modified().ok()?, metadata.len(), path))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.sort();
    files.pop();
    for (_, len, path) in files {
        if total <= cap {
            break;
        }
        match fs::remove_file(&path) {
            Ok(()) => total -= len,
            Err(error) => tracing::debug!(%error, "columnar file in use; keeping it for now"),
        }
    }
}

fn remove_stray_partials(columnar_dir: &Path) {
    let Ok(entries) = fs::read_dir(columnar_dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        let name = path.to_string_lossy();
        if name.ends_with(".partial") {
            remove_partial(&path);
        } else if name.ends_with(".partial.wal")
            && let Err(error) = fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not delete an orphan partial log");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::scan_expression;
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    const SCAN: &str = "read_csv_auto('x.csv')";
    const VERSION: &str = "v1.5.4";

    fn key(path: &Path, scan: &str, version: &str) -> ColumnarKey {
        ColumnarKey::new(path, scan, version).unwrap()
    }

    #[test]
    fn key_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a,b\n1,2\n").unwrap();
        let first = key(&file, SCAN, VERSION);
        assert_eq!(first, key(&file, SCAN, VERSION));
        assert_eq!(first.as_str().len(), 64);
        assert!(
            first
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }

    #[test]
    fn key_changes_with_size_mtime_path_scan_or_version() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a,b\n1,2\n").unwrap();
        let base = key(&file, SCAN, VERSION);

        assert_ne!(base, key(&file, "read_csv_auto('y.csv')", VERSION));
        assert_ne!(base, key(&file, SCAN, "v1.5.5"));

        let other = dir.path().join("b.csv");
        fs::copy(&file, &other).unwrap();
        assert_ne!(base, key(&other, SCAN, VERSION));

        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified + Duration::from_secs(1))
            .unwrap();
        let touched = key(&file, SCAN, VERSION);
        assert_ne!(base, touched);

        fs::write(&file, "a,b\n1,2\n3,4\n").unwrap();
        File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified + Duration::from_secs(1))
            .unwrap();
        assert_ne!(touched, key(&file, SCAN, VERSION));
    }

    #[test]
    fn paths_live_under_the_columnar_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a\n1\n").unwrap();
        let key = key(&file, SCAN, VERSION);
        let cache = dir.path().join("cache");
        fs::create_dir_all(cache.join("columnar")).unwrap();

        assert_eq!(
            key.final_path(&cache),
            cache
                .join("columnar")
                .join(format!("{}.duckdb", key.as_str()))
        );
        assert!(
            key.partial_path(&cache)
                .to_string_lossy()
                .ends_with(".duckdb.partial")
        );
        assert!(!key.is_ready(&cache));
        fs::write(key.final_path(&cache), b"").unwrap();
        assert!(key.is_ready(&cache));
    }

    const WAIT: Duration = Duration::from_secs(60);

    fn start(cache: &Path) -> (ColumnarWorker, Receiver<String>) {
        let (sender, ready) = mpsc::channel();
        let on_ready = Box::new(move |source_id: String| {
            sender.send(source_id).unwrap();
        });
        let worker = ColumnarWorker::start(cache, &cache.join("duckdb-tmp"), on_ready);
        (worker, ready)
    }

    fn start_with_cap(cache: &Path, cap: u64) -> (ColumnarWorker, Receiver<String>) {
        let (sender, ready) = mpsc::channel();
        let on_ready = Box::new(move |source_id: String| {
            sender.send(source_id).unwrap();
        });
        let worker = ColumnarWorker::with_cap(cap, cache, &cache.join("duckdb-tmp"), on_ready);
        (worker, ready)
    }

    fn job(dir: &Path, name: &str, body: &str) -> ColumnarJob {
        let file = dir.join(name);
        fs::write(&file, body).unwrap();
        let scan = scan_expression(&file.to_string_lossy()).unwrap();
        ColumnarJob {
            source_id: name.to_owned(),
            key: key(&file, &scan, VERSION),
            source_path: file,
            scan_expression: scan,
        }
    }

    fn snapshot(conn: &Connection, from: &str) -> (Vec<(String, String)>, Vec<String>) {
        let columns = conn
            .prepare(&format!("DESCRIBE SELECT * FROM {from}"))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let rows = conn
            .prepare(&format!(
                "SELECT CAST(t AS VARCHAR) FROM (SELECT * FROM {from}) t"
            ))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        (columns, rows)
    }

    #[test]
    fn import_matches_live_view() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let (mut worker, ready) = start(&cache);
        let csv = "id,price,name,born,active\n1,1.5,alpha,2020-01-02,true\n2,,beta,2021-03-04,false\n3,3.25,,2022-05-06,true\n";
        let job = job(dir.path(), "a.csv", csv);
        let (key, scan) = (job.key.clone(), job.scan_expression.clone());
        worker.enqueue(job);

        assert_eq!(ready.recv_timeout(WAIT).unwrap(), "a.csv");
        worker.shutdown();
        assert!(key.is_ready(&cache));
        assert!(!key.partial_path(&cache).exists());
        let cached = Connection::open(key.final_path(&cache)).unwrap();
        let live = Connection::open_in_memory().unwrap();
        assert_eq!(snapshot(&cached, "data"), snapshot(&live, &scan));
    }

    #[test]
    fn leftover_partial_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let job = job(dir.path(), "a.csv", "a,b\n1,2\n");
        let partial = job.key.partial_path(&cache);
        fs::create_dir_all(cache.join("columnar")).unwrap();
        fs::write(&partial, b"not a database").unwrap();

        let (mut worker, ready) = start(&cache);
        assert!(!partial.exists());
        fs::write(&partial, b"still not a database").unwrap();
        let key = job.key.clone();
        worker.enqueue(job);

        assert_eq!(ready.recv_timeout(WAIT).unwrap(), "a.csv");
        worker.shutdown();
        assert!(!partial.exists());
        let cached = Connection::open(key.final_path(&cache)).unwrap();
        let count: i64 = cached
            .query_row("SELECT count(*) FROM data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn orphan_partial_wal_is_swept() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let columnar = cache.join("columnar");
        fs::create_dir_all(&columnar).unwrap();
        let wal = columnar.join("x.duckdb.partial.wal");
        fs::write(&wal, b"orphan log").unwrap();

        let (mut worker, _ready) = start(&cache);
        worker.shutdown();

        assert!(!wal.exists());
    }

    /// A 10-byte stand-in cache file last used `age` ago.
    fn cache_file(dir: &Path, cache: &Path, name: &str, age: Duration) -> ColumnarKey {
        let source = dir.join(name);
        fs::write(&source, "a\n1\n").unwrap();
        let key = key(&source, name, VERSION);
        fs::create_dir_all(cache.join("columnar")).unwrap();
        fs::write(key.final_path(cache), b"0123456789").unwrap();
        File::options()
            .write(true)
            .open(key.final_path(cache))
            .unwrap()
            .set_modified(SystemTime::now() - age)
            .unwrap();
        key
    }

    #[test]
    fn eviction_removes_least_recent_over_cap() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let day = Duration::from_secs(86_400);
        let oldest = cache_file(dir.path(), &cache, "a.csv", 3 * day);
        let middle = cache_file(dir.path(), &cache, "b.csv", 2 * day);
        let newest = cache_file(dir.path(), &cache, "c.csv", day);
        oldest.touch(&cache);

        let (mut worker, ready) = start_with_cap(&cache, 25);
        assert!(oldest.is_ready(&cache), "mounting made it the most recent");
        assert!(!middle.is_ready(&cache), "startup evicts the least recent");
        assert!(newest.is_ready(&cache));

        let job = job(dir.path(), "d.csv", "a\n1\n");
        let imported = job.key.clone();
        worker.enqueue(job);
        assert_eq!(ready.recv_timeout(WAIT).unwrap(), "d.csv");
        worker.shutdown();
        assert!(!oldest.is_ready(&cache) && !newest.is_ready(&cache));
        assert!(
            imported.is_ready(&cache),
            "the newest file stays over the cap"
        );
    }

    #[test]
    fn failed_key_is_not_retried() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let (mut worker, ready) = start(&cache);
        let missing = dir.path().join("missing.csv");
        let scan = scan_expression(&missing.to_string_lossy()).unwrap();
        let anchor = job(dir.path(), "anchor.csv", "a\n1\n");
        let failing = || ColumnarJob {
            source_id: "missing.csv".to_owned(),
            key: key(&anchor.source_path, &scan, VERSION),
            source_path: missing.clone(),
            scan_expression: scan.clone(),
        };
        let failed_key = failing().key;

        worker.enqueue(failing());
        worker.enqueue(job(dir.path(), "first.csv", "a\n1\n"));
        assert_eq!(ready.recv_timeout(WAIT).unwrap(), "first.csv");
        assert!(!failed_key.is_ready(&cache));
        assert!(!failed_key.partial_path(&cache).exists());

        fs::write(&missing, "a\n1\n").unwrap();
        worker.enqueue(failing());
        worker.enqueue(job(dir.path(), "second.csv", "a\n2\n"));
        assert_eq!(ready.recv_timeout(WAIT).unwrap(), "second.csv");
        worker.shutdown();
        assert!(!failed_key.is_ready(&cache));
        assert!(ready.try_recv().is_err());
    }
}
