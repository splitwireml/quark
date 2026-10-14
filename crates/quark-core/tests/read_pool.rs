//! Pooled readers over real TCP: pages are served beside slow queries, and a dropped request frees only its own reader.
// Helpers outside `#[test]` fns are not covered by clippy.toml's allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use duckdb::Connection;
use quark_core::router;
use quark_core::runtime::AppState;
use quark_core::runtime::dirs::Dirs;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

const SLOW_SQL: &str = "SELECT sum(i) FROM range(10000000000) t(i)";
const QUICK_SQL: &str = "SELECT a FROM data";

/// The API on an ephemeral loopback port, with one database node `legacy`.
struct Server {
    address: SocketAddr,
    runtime: Option<Runtime>,
}

impl Server {
    fn start(root: &Path) -> Self {
        let dirs = Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        };
        fs::create_dir_all(&dirs.data).unwrap();
        let file = root.join("legacy.duckdb");
        Connection::open(&file)
            .unwrap()
            .execute_batch("CREATE TABLE data AS SELECT range AS a FROM range(3)")
            .unwrap();
        let entry = json!([{
            "id": "legacy", "name": "LEGACY", "kind": "duckdb", "source": file.to_string_lossy(),
        }]);
        fs::write(dirs.registry_file(), entry.to_string()).unwrap();
        fs::write(dirs.projects_file(), "[]").unwrap();
        let state = AppState::load(dirs).unwrap();

        let runtime = Runtime::new().unwrap();
        let listener = runtime.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
        let address = listener.local_addr().unwrap();
        runtime.spawn(async move { axum::serve(listener, router(state)).await });
        Self {
            address,
            runtime: Some(runtime),
        }
    }

    /// Sends a SQL page request and returns the open connection without reading the answer.
    fn send_sql(&self, sql: &str) -> TcpStream {
        self.send_post("/api/nodes/legacy/sql", &json!({ "sql": sql }))
    }

    /// Posts a JSON body and returns the open connection without reading the answer.
    fn send_post(&self, path: &str, body: &Value) -> TcpStream {
        let body = body.to_string();
        let mut stream = TcpStream::connect(self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.address,
            body.len()
        )
        .unwrap();
        stream
    }

    /// Runs a SQL page request to completion and returns its status and JSON body.
    fn answer_of(&self, sql: &str) -> (u16, Value) {
        let mut response = String::new();
        self.send_sql(sql).read_to_string(&mut response).unwrap();
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|status| status.parse().ok())
            .unwrap_or_else(|| panic!("no status line in {response:?}"));
        let body = response.split_once("\r\n\r\n").map_or("", |(_, body)| body);
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
    }
}

/// True when the server has sent nothing yet, so the request is still running.
fn is_pending(stream: &mut TcpStream) -> bool {
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    matches!(
        stream.read(&mut [0; 1]),
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
    )
}

/// A query that cannot use more than one thread, so two of them overlap only on two readers.
fn single_threaded_sql(value: u32) -> String {
    format!("SELECT list_sum(list_transform(range(12000000), x -> x * 0)) + {value} AS value")
}

#[test]
fn page_is_served_while_a_slow_query_runs() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());
    assert_eq!(server.answer_of(QUICK_SQL).0, 200);

    let mut slow = server.send_sql(SLOW_SQL);
    thread::sleep(Duration::from_secs(2));
    let started_at = Instant::now();
    let (status, page) = server.answer_of(QUICK_SQL);
    let waited = started_at.elapsed();

    assert_eq!(status, 200);
    assert_eq!(page["rows"].as_array().map(Vec::len), Some(3));
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
    assert!(
        is_pending(&mut slow),
        "the slow query should still be running"
    );
}

#[test]
fn dropped_request_frees_only_its_reader() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());
    let mut kept = server.send_sql(SLOW_SQL);
    let dropped = server.send_sql(SLOW_SQL);
    thread::sleep(Duration::from_millis(500));
    drop(dropped);

    // Both readers were busy, so this page can only run on the one the dropped request freed.
    let started_at = Instant::now();
    let (status, _) = server.answer_of(QUICK_SQL);
    let waited = started_at.elapsed();

    assert_eq!(status, 200);
    assert!(waited < Duration::from_secs(1), "waited {waited:?}");
    assert!(is_pending(&mut kept), "the other query was interrupted");
}

#[test]
fn burst_of_requests_with_drops_never_deadlocks() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());

    thread::scope(|scope| {
        for client in 0..12u32 {
            let server = &server;
            scope.spawn(move || {
                if client % 2 == 0 {
                    let slow = server.send_sql(SLOW_SQL);
                    thread::sleep(Duration::from_millis(u64::from(client) * 20));
                    drop(slow);
                } else if client % 4 == 1 {
                    assert_eq!(server.answer_of(QUICK_SQL).0, 200);
                } else {
                    let (status, page) = server.answer_of(&single_threaded_sql(client));
                    assert_eq!(status, 200);
                    assert_eq!(page["rows"], json!([{ "value": client }]));
                }
            });
        }
    });

    let started_at = Instant::now();
    let (status, _) = server.answer_of(QUICK_SQL);
    let waited = started_at.elapsed();
    assert_eq!(status, 200);
    assert!(waited < Duration::from_secs(1), "waited {waited:?}");
}

/// The counterpart of Python's `test_project_sql_requests_serialize_shared_workspace`: Python must
/// serialize requests on its one connection so their results never mix; here every request owns a
/// pooled reader, so concurrent requests overlap in time and still each get their own rows.
#[test]
fn concurrent_sql_requests_overlap_and_keep_their_own_results() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());
    let solo_started = Instant::now();
    assert_eq!(server.answer_of(&single_threaded_sql(0)).0, 200);
    let solo = solo_started.elapsed();
    assert!(
        solo > Duration::from_millis(150),
        "query too quick: {solo:?}"
    );

    let started_at = Instant::now();
    let answers: Vec<_> = thread::scope(|scope| {
        let workers: Vec<_> = [1, 2]
            .map(|value| {
                let server = &server;
                scope.spawn(move || server.answer_of(&single_threaded_sql(value)))
            })
            .into();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    let together = started_at.elapsed();

    for (answer, value) in answers.iter().zip([1, 2]) {
        assert_eq!(answer.0, 200);
        assert_eq!(answer.1["rows"], json!([{ "value": value }]));
    }
    // Two serial runs would take about twice `solo`; real overlap stays near one.
    assert!(
        together < solo.mul_f64(1.7),
        "{together:?} vs solo {solo:?}"
    );
}

#[test]
fn page_is_served_before_a_running_export_finishes() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());
    assert_eq!(server.answer_of(QUICK_SQL).0, 200);

    // The export answers only once its file is written, so no bytes on its socket means it is running.
    let mut export = server.send_post(
        "/api/exports",
        &json!({"format": "csv", "sheets": [{
            "node_id": "legacy", "name": "Big",
            "sql": "SELECT sum(i) AS total FROM range(2000000000) t(i)",
        }]}),
    );
    thread::sleep(Duration::from_millis(200));
    assert!(is_pending(&mut export), "the export should be running");

    let (status, page) = server.answer_of(QUICK_SQL);
    assert_eq!(status, 200);
    assert_eq!(page["rows"].as_array().map(Vec::len), Some(3));
    assert!(
        is_pending(&mut export),
        "the page should be answered while the export is still running"
    );

    export
        .set_read_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    let mut response = String::new();
    export.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response:.200}");
    assert!(response.contains("total"), "{response:.200}");
}
