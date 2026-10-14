//! User-facing API errors, rendered as FastAPI-shaped `{"detail": ...}` bodies.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("{status}: {detail}")]
pub struct ApiError {
    status: StatusCode,
    detail: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize)]
struct Body<'a> {
    detail: &'a str,
}

impl ApiError {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, detail)
    }

    pub fn unprocessable(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, detail)
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, detail)
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Body {
            detail: &self.detail,
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests;
