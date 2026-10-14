//! The loopback HTTP layer: the router, request logging, panic handling, extractors and request guards.

use std::any::Any;
use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tower_http::catch_panic::CatchPanicLayer;

use crate::error::ApiError;
use crate::features::{
    charts, exports, find, joins, pages, projects, sources, stats, values, workbooks,
};
use crate::runtime::AppState;

pub mod deps;
mod fallback;
pub mod secure;

#[cfg(test)]
mod tests;

/// Every route the desktop build serves. An unknown path answers 404 and a wrong method 405.
pub fn router(state: AppState) -> Router {
    guarded(
        Router::new()
            .merge(projects::routes())
            .merge(sources::routes())
            .merge(pages::routes())
            .merge(values::routes())
            .merge(joins::routes())
            .merge(stats::routes())
            .merge(find::routes())
            .merge(charts::routes())
            .merge(exports::routes())
            .merge(workbooks::routes())
            .fallback(fallback::unknown_route)
            .method_not_allowed_fallback(fallback::wrong_method),
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

fn panic_response(_: Box<dyn Any + Send>) -> Response {
    tracing::error!("a request handler panicked");
    ApiError::internal("Internal error").into_response()
}
