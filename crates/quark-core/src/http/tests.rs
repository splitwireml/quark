//! Tests for request logging, panic recovery and unknown-route answers in the router.

use super::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use std::cell::RefCell;
use std::io::Write;
use std::sync::Once;
use tower::ServiceExt;

thread_local! {
    static CAPTURED: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Writes log output into the current thread's buffer, so parallel tests never see each other's lines.
struct ThreadCapture;

impl Write for ThreadCapture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        CAPTURED.with_borrow_mut(|buffer| buffer.extend_from_slice(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A process-wide subscriber: a thread-scoped one loses callsites that other threads registered first.
fn capture_logs() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(|| ThreadCapture)
                .finish(),
        )
        .unwrap();
    });
    CAPTURED.with_borrow_mut(Vec::clear);
}

#[tokio::test]
async fn requests_are_logged_without_secrets() {
    let root = tempfile::tempdir().unwrap();
    let dirs = crate::runtime::dirs::Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    std::fs::create_dir_all(&dirs.data).unwrap();
    let file = root.path().join("legacy.csv");
    std::fs::write(&file, "a,b\n1,x\n").unwrap();
    let registry = serde_json::json!([
        {"id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy()}
    ]);
    std::fs::write(dirs.registry_file(), registry.to_string()).unwrap();
    std::fs::write(dirs.projects_file(), "[]").unwrap();
    let app = router(AppState::load(dirs).unwrap());
    capture_logs();

    let missing = "/api/nodes/legacy/datasets/bm9wZQ/query";
    for (method, path, status) in [("GET", "/api/nodes", 200), ("POST", missing, 404)] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", "Bearer secret-token")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status);
    }

    let logged = String::from_utf8(CAPTURED.with_borrow(Clone::clone)).unwrap();
    for expected in ["/api/nodes ", missing, "200", "404", "Dataset not found"] {
        assert!(
            logged.contains(expected),
            "missing {expected:?} in {logged}"
        );
    }
    assert!(!logged.contains("secret-token"), "{logged}");
}

#[tokio::test]
async fn panicking_handler_becomes_internal_error() {
    async fn boom() -> &'static str {
        panic!("boom")
    }
    let app: Router = guarded(Router::new().route("/boom", get(boom)));

    let response = app
        .oneshot(Request::get("/boom").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&bytes[..], br#"{"detail":"Internal error"}"#);
}

/// The error body of `request` against a router over an empty data folder.
async fn answer(method: &str, path: &str) -> (StatusCode, serde_json::Value) {
    let root = tempfile::tempdir().unwrap();
    let dirs = crate::runtime::dirs::Dirs {
        data: root.path().join("data"),
        cache: root.path().join("cache"),
    };
    let app = router(AppState::load(dirs).unwrap());
    let request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn unknown_api_path_is_404_not_found() {
    for path in ["/api/export", "/api/nodes/upload/extra/parts", "/elsewhere"] {
        let (status, body) = answer("GET", path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body, serde_json::json!({"detail": "Not Found"}), "{path}");
    }
}

#[tokio::test]
async fn wrong_method_is_405() {
    for (method, path) in [("DELETE", "/api/nodes"), ("GET", "/api/nodes/upload")] {
        let (status, body) = answer(method, path).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {path}");
        assert_eq!(
            body,
            serde_json::json!({"detail": "Method Not Allowed"}),
            "{method} {path}"
        );
    }
}
