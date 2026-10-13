//! Tests for listing and creating projects, in the service and over HTTP.

use super::service::{create_project, list_projects};
use crate::api::router;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use crate::runtime::registry::load_projects;
use crate::runtime::state::fixtures::*;
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tower::ServiceExt;

#[test]
fn projects_list_default_first_with_counts() {
    let root = tempfile::tempdir().unwrap();
    let mut gone = entry(root.path(), "gone", Some("p"));
    gone["source"] = json!(root.path().join("missing.csv").to_string_lossy());
    let state = started(
        root.path(),
        &[
            entry(root.path(), "legacy", None),
            entry(root.path(), "one", Some("p")),
            entry(root.path(), "two", Some("p")),
            gone,
        ],
        &[project("p")],
    );

    let listed = serde_json::to_value(list_projects(&state)).unwrap();

    assert_eq!(
        listed,
        json!([
            {"id": "default", "name": "Default", "node_id": "project_default", "source_count": 1},
            {"id": "p", "name": "Project p", "node_id": "project_p", "source_count": 2},
        ])
    );
}

#[test]
fn create_project_validation() {
    let root = tempfile::tempdir().unwrap();
    let state = started(root.path(), &[], &[]);
    let required = (422, "Project name is required".to_owned());
    for bad in ["", "   \t", &"x".repeat(201)] {
        assert_eq!(failure(create_project(&state, bad)), required, "{bad:?}");
    }
    assert!(load_projects(&state.0.dirs.projects_file()).is_empty());

    let created = create_project(&state, "  Quarterly  ").unwrap();
    assert_eq!(created.project.name, "Quarterly");
    assert_eq!(created.source_count, 0);
    assert_eq!(created.project.id.len(), 32);
    assert!(
        created
            .project
            .id
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    );
    assert_eq!(
        created.project.node_id,
        format!("project_{}", created.project.id)
    );
    create_project(&state, &"\u{1F600}".repeat(200)).unwrap();

    let saved = load_projects(&state.0.dirs.projects_file());
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0], created.project);
    assert!(saved.iter().all(|saved| saved.id != "default"));
    assert_eq!(list_projects(&state).len(), 3);
}

/// A started app over a temp data folder holding the given registry and projects.
fn app(root: &std::path::Path, registry: &[Value], projects: &[Value]) -> Router {
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    fs::create_dir_all(&dirs.data).unwrap();
    fs::write(dirs.registry_file(), json!(registry).to_string()).unwrap();
    fs::write(dirs.projects_file(), json!(projects).to_string()).unwrap();
    router(AppState::load(dirs).unwrap())
}

async fn send(app: &Router, method: Method, uri: &str, body: Option<&str>) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(text) => {
            request = request.header("content-type", "application/json");
            Body::from(text.to_owned())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn get_json(app: &Router, uri: &str) -> (u16, Value) {
    send(app, Method::GET, uri, None).await
}

#[tokio::test]
async fn projects_routes() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path(), &[], &[]);

    let (status, listed) = get_json(&app, "/api/projects").await;
    assert_eq!(status, 200);
    assert_eq!(
        listed,
        json!([{"id": "default", "name": "Default", "node_id": "project_default", "source_count": 0}])
    );

    let (status, created) = send(
        &app,
        Method::POST,
        "/api/projects",
        Some(r#"{"name":"  Quarterly  "}"#),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(created["name"], "Quarterly");
    assert_eq!(created["source_count"], 0);
    assert_eq!(
        created["node_id"],
        format!("project_{}", created["id"].as_str().unwrap())
    );
    let (_, listed) = get_json(&app, "/api/projects").await;
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[1], created);

    let (status, blank) = send(
        &app,
        Method::POST,
        "/api/projects",
        Some(r#"{"name":"  "}"#),
    )
    .await;
    assert_eq!(
        (status, blank),
        (422, json!({"detail": "Project name is required"}))
    );
    for bad in [
        r#"{}"#,
        r#"{"name":"x","extra":1}"#,
        r#"{"name":"#,
        r#"{"name":3}"#,
    ] {
        let (status, body) = send(&app, Method::POST, "/api/projects", Some(bad)).await;
        assert_eq!(status, 422, "{bad}");
        assert!(body["detail"].is_string(), "{bad}");
    }
    let request = Request::post("/api/projects")
        .body(Body::from("{}"))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
