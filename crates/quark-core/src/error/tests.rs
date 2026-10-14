//! Tests for the `{"detail": ...}` rendering of `ApiError`.

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
