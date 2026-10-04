//! A client that goes away must free the engine it was using, over a real TCP connection.
// Helpers outside `#[test]` fns are not covered by clippy.toml's allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use duckdb::Connection;
use quark_core::api::router;
use quark_core::engine::Dirs;
use quark_core::state::AppState;
use serde_json::json;
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
        // A database source, so no columnar import swaps the engine out mid-test.
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
        let body = json!({ "sql": sql }).to_string();
        let mut stream = TcpStream::connect(self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(
            stream,
            "POST /api/nodes/legacy/sql HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.address,
            body.len()
        )
        .unwrap();
        stream
    }

    /// Runs a SQL page request to completion and returns its status code.
    fn status_of(&self, sql: &str) -> u16 {
        let mut stream = self.send_sql(sql);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
            .split_whitespace()
            .nth(1)
            .and_then(|status| status.parse().ok())
            .unwrap_or_else(|| panic!("no status line in {response:?}"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
    }
}

#[test]
fn aborted_request_frees_engine_quickly() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());
    assert_eq!(server.status_of("SELECT a + 1 FROM data"), 200);

    let slow = server.send_sql(SLOW_SQL);
    thread::sleep(Duration::from_millis(200));
    drop(slow);
    let dropped_at = Instant::now();
    let status = server.status_of(QUICK_SQL);
    let waited = dropped_at.elapsed();

    assert_eq!(status, 200);
    assert!(waited < Duration::from_millis(100), "waited {waited:?}");
}

#[test]
fn rapid_aborts_leave_engine_healthy() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path());

    for cycle in 0..50 {
        let slow = server.send_sql(SLOW_SQL);
        thread::sleep(Duration::from_millis(cycle % 5 * 10));
        drop(slow);
    }

    assert_eq!(server.status_of(QUICK_SQL), 200);
}
