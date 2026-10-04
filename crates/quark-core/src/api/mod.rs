//! The loopback HTTP API: routes, request parsing, request logging and panic handling.

use std::any::Any;
use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::http::{StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use tower_http::catch_panic::CatchPanicLayer;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

mod query;
mod read;
mod sources;

/// Every route the desktop build serves. Other `/api/*` requests, including a wrong method on a known path, answer 501; the rest 404.
pub fn router(state: AppState) -> Router {
    guarded(
        Router::new()
            .merge(read::routes())
            .merge(sources::routes())
            .merge(query::routes())
            .fallback(unknown_route)
            .method_not_allowed_fallback(unknown_route),
    )
    .layer(middleware::from_fn(log_request))
    .with_state(state)
}

/// Error bodies are a short `{"detail": ...}`; anything larger is not read back for logging.
const ERROR_BODY_LIMIT: usize = 64 * 1024;

/// Logs method, path (never the query string), status and duration; failures also log their `detail`.
async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status();
    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(%method, %path, status = status.as_u16(), elapsed_ms, "request");
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }
    let (parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, ERROR_BODY_LIMIT).await else {
        return Response::from_parts(parts, Body::empty());
    };
    if let Ok(error) = serde_json::from_slice::<ErrorBody>(&bytes) {
        tracing::warn!(%path, status = status.as_u16(), detail = %error.detail, "request failed");
    }
    Response::from_parts(parts, Body::from(bytes))
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    detail: String,
}

fn guarded<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router.layer(CatchPanicLayer::custom(panic_response))
}

async fn unknown_route(uri: Uri) -> ApiError {
    if uri.path().starts_with("/api/") {
        ApiError::not_implemented("Not in the desktop build yet")
    } else {
        ApiError::not_found("Not Found")
    }
}

fn panic_response(_: Box<dyn Any + Send>) -> Response {
    tracing::error!("a request handler panicked");
    ApiError::internal("Internal error").into_response()
}

/// Runs `work` off the async runtime; a task that panics or is cancelled becomes a 500.
pub(crate) async fn blocking<T, F>(work: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> ApiResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        tracing::error!(%error, "blocking task failed");
        ApiError::internal("Internal error")
    })?
}

/// A JSON body whose rejections (syntax, content type, schema) are 422s; an over-limit body stays a 413.
pub(crate) struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(value)| Self(value))
            .map_err(|rejection| match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => {
                    ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, rejection.body_text())
                }
                _ => ApiError::unprocessable(rejection.body_text()),
            })
    }
}

#[cfg(test)]
mod tests {
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
        let dirs = crate::engine::Dirs {
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

    #[tokio::test]
    async fn panicking_blocking_task_becomes_internal_error() {
        let error = blocking(|| -> ApiResult<()> { panic!("boom") })
            .await
            .unwrap_err();

        assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.detail(), "Internal error");
    }
}
