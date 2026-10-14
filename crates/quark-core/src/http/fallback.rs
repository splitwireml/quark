//! Answers for paths and methods the router does not serve, worded as FastAPI words them.

use axum::http::StatusCode;

use crate::error::ApiError;

/// An unknown path: 404 `Not Found`.
pub(super) async fn unknown_route() -> ApiError {
    ApiError::not_found("Not Found")
}

/// A known path called with a method it does not serve: 405 `Method Not Allowed`.
pub(super) async fn wrong_method() -> ApiError {
    ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")
}
