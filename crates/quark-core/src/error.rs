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

    pub fn not_implemented(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_IMPLEMENTED, detail)
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
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    /// Body reads on an in-memory body are immediately ready, so one poll suffices.
    fn body_bytes(response: Response) -> Vec<u8> {
        let mut future = std::pin::pin!(axum::body::to_bytes(response.into_body(), usize::MAX));
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(bytes) => bytes.unwrap().to_vec(),
            Poll::Pending => panic!("in-memory body was not ready"),
        }
    }

    #[test]
    fn renders_status_and_detail_json() {
        let response = ApiError::not_found("Project not found").into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body: serde_json::Value = serde_json::from_slice(&body_bytes(response)).unwrap();
        assert_eq!(body, serde_json::json!({"detail": "Project not found"}));
    }

    #[test]
    fn not_implemented_is_501() {
        let error = ApiError::not_implemented("Not in the desktop build yet");
        assert_eq!(error.status(), StatusCode::NOT_IMPLEMENTED);
        assert_eq!(error.detail(), "Not in the desktop build yet");
        assert_eq!(
            error.to_string(),
            "501 Not Implemented: Not in the desktop build yet"
        );
    }
}
