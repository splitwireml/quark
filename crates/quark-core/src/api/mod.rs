//! The loopback HTTP API: routes, request parsing and panic handling.

use std::any::Any;

use axum::Router;
use axum::extract::{FromRequest, Request};
use axum::http::Uri;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use tower_http::catch_panic::CatchPanicLayer;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

mod read;

/// Every route the desktop build serves. Other `/api/*` requests, including a wrong method on a known path, answer 501; the rest 404.
pub fn router(state: AppState) -> Router {
    guarded(
        Router::new()
            .merge(read::routes())
            .fallback(unknown_route)
            .method_not_allowed_fallback(unknown_route),
    )
    .with_state(state)
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

/// A JSON body whose every rejection (syntax, content type, schema) is a 422.
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
            .map_err(|rejection| ApiError::unprocessable(rejection.body_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

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
