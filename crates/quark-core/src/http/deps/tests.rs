//! Tests for the extractors: output format, project and engine lookup, and blocking-task failures.

use super::*;
use crate::runtime::dirs::Dirs;
use crate::runtime::registry::DEFAULT_PROJECT_ID;
use axum::Router;
use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::http::Request;
use axum::routing::get;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tower::ServiceExt;

async fn format_of(accept: &[&str]) -> Format {
    let mut request = Request::get("/");
    for value in accept {
        request = request.header("accept", *value);
    }
    let (mut parts, _) = request.body(()).unwrap().into_parts();
    Format::from_request_parts(&mut parts, &()).await.unwrap()
}

#[tokio::test]
async fn format_prefers_arrow_when_accepted() {
    assert_eq!(format_of(&[]).await, Format::Json);
    assert_eq!(format_of(&["application/json"]).await, Format::Json);
    let arrow = format!("text/html, {ARROW_MEDIA_TYPE};q=0.9");
    assert_eq!(format_of(&[&arrow]).await, Format::Arrow);
    assert_eq!(
        format_of(&["application/json", ARROW_MEDIA_TYPE]).await,
        Format::Arrow
    );
}

/// A started app with one CSV node `legacy`, served by routes that use the extractors.
fn app(root: &std::path::Path) -> Router {
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    fs::create_dir_all(&dirs.data).unwrap();
    let file = root.join("legacy.csv");
    fs::write(&file, "a\n1\n").unwrap();
    let entry = json!([{
        "id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy(),
    }]);
    fs::write(dirs.registry_file(), entry.to_string()).unwrap();
    fs::write(dirs.projects_file(), "[]").unwrap();
    let state = AppState::load(dirs).unwrap();
    Router::new()
        .route(
            "/engine/{node_id}",
            get(|EngineFor(_): EngineFor| async { "engine" }),
        )
        .route(
            "/project/{project_id}",
            get(|Project(project): Project| async move { project.id }),
        )
        .with_state(state)
}

async fn get_text(app: &Router, uri: &str) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

#[tokio::test]
async fn engine_for_unknown_node_is_404() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let (status, body) = get_text(&app, "/engine/nope").await;
    assert_eq!(status, 404);
    assert_eq!(body, json!({"detail": "Node not found"}));
    let (status, body) = get_text(&app, "/engine/legacy").await;
    assert_eq!((status, body), (200, json!("engine")));
}

#[tokio::test]
async fn project_for_unknown_id_is_404() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());

    let (status, body) = get_text(&app, "/project/nope").await;
    assert_eq!(status, 404);
    assert_eq!(body, json!({"detail": "Project not found"}));
    let (status, body) = get_text(&app, &format!("/project/{DEFAULT_PROJECT_ID}")).await;
    assert_eq!((status, body), (200, json!(DEFAULT_PROJECT_ID)));
}

#[tokio::test]
async fn panicking_blocking_task_becomes_internal_error() {
    let error = blocking(|| -> ApiResult<()> { panic!("boom") })
        .await
        .unwrap_err();

    assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.detail(), "Internal error");
}
