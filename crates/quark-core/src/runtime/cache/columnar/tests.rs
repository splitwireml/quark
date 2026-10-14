//! Tests for the columnar worker: imports, eviction, failures and shutdown.

use super::*;
use crate::shared::sql::scan_expression;
use std::fs::File;
use std::time::{Duration, Instant, SystemTime};

const VERSION: &str = "v1.5.4";

fn key(path: &Path, scan: &str, version: &str) -> ColumnarKey {
    ColumnarKey::new(path, scan, version).unwrap()
}

const WAIT: Duration = Duration::from_secs(60);

fn start(cache: &Path) -> (ColumnarWorker, Receiver<String>) {
    let (sender, ready) = mpsc::channel();
    let on_ready = Box::new(move |source_id: String, _key: ColumnarKey| {
        sender.send(source_id).unwrap();
    });
    let worker = ColumnarWorker::start(cache, &cache.join("duckdb-tmp"), on_ready);
    (worker, ready)
}

fn start_with_cap(cache: &Path, cap: u64) -> (ColumnarWorker, Receiver<String>) {
    let (sender, ready) = mpsc::channel();
    let on_ready = Box::new(move |source_id: String, _key: ColumnarKey| {
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

#[test]
fn stray_wal_files_are_swept() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let columnar = cache.join("columnar");
    fs::create_dir_all(&columnar).unwrap();
    let stray = columnar.join("gone.duckdb.wal");
    let kept_file = columnar.join("live.duckdb");
    let kept_wal = columnar.join("live.duckdb.wal");
    fs::write(&stray, b"log of a deleted cache").unwrap();
    fs::write(&kept_file, b"cache").unwrap();
    fs::write(&kept_wal, b"log of a live cache").unwrap();

    let (mut worker, _ready) = start(&cache);
    worker.shutdown();

    assert!(!stray.exists());
    assert!(kept_file.exists() && kept_wal.exists());
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
    let middle_wal = PathBuf::from(format!("{}.wal", middle.final_path(&cache).display()));
    fs::write(&middle_wal, b"log").unwrap();

    let (mut worker, ready) = start_with_cap(&cache, 25);
    assert!(oldest.is_ready(&cache), "mounting made it the most recent");
    assert!(!middle.is_ready(&cache), "startup evicts the least recent");
    assert!(
        !middle_wal.exists(),
        "eviction removes the file's write-ahead log"
    );
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

#[test]
fn default_cap_is_ten_gibibytes() {
    assert_eq!(CACHE_CAP, 10 * 1024_u64.pow(3));
}

/// A scan that never finishes on its own, so only an interrupt can end the import.
const ENDLESS_SCAN: &str = "(SELECT sum(i) FROM range(10000000000) t(i))";

fn endless_job(dir: &Path) -> ColumnarJob {
    let mut job = job(dir, "endless.csv", "a\n1\n");
    job.scan_expression = ENDLESS_SCAN.to_owned();
    job
}

#[test]
fn shutdown_interrupts_a_running_import() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let job = endless_job(dir.path());
    let key = job.key.clone();
    let (mut worker, ready) = start(&cache);
    worker.enqueue(job);
    let started = Instant::now();
    while !key.partial_path(&cache).exists() {
        assert!(started.elapsed() < WAIT, "the import never started");
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(300));

    let stopping = Instant::now();
    worker.shutdown();

    assert!(stopping.elapsed() < Duration::from_secs(2));
    assert!(!key.is_ready(&cache));
    assert!(!key.partial_path(&cache).exists());
    assert!(ready.try_recv().is_err());
}

#[test]
fn shutdown_between_statements_still_stops_the_import() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let job = endless_job(dir.path());
    let key = job.key.clone();
    let (mut worker, ready) = start(&cache);
    let (reached_sender, reached) = mpsc::channel();
    let (release, release_receiver) = mpsc::channel();
    *worker.shared.hold.lock().unwrap() = Some(Hold {
        reached: reached_sender,
        release: release_receiver,
    });
    worker.enqueue(job);
    reached.recv_timeout(WAIT).unwrap();

    // Let the import go on only after shutdown has begun and its first interrupt is spent.
    let shared = Arc::clone(&worker.shared);
    thread::spawn(move || {
        while !shared.is_stopping() {
            thread::sleep(Duration::from_millis(1));
        }
        thread::sleep(Duration::from_millis(50));
        release.send(()).ok();
    });
    let (stopped_sender, stopped) = mpsc::channel();
    thread::spawn(move || {
        worker.shutdown();
        stopped_sender.send(()).ok();
    });

    stopped
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown must return while the import is between statements");
    assert!(!key.is_ready(&cache));
    assert!(!key.partial_path(&cache).exists());
    assert!(ready.try_recv().is_err());
}
