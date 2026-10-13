//! Tests for listing sources, views and datasets, in the services and over HTTP.

use super::service::{datasets, legacy_nodes, project_sources, project_views, source_detail};
use crate::api::router;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use crate::runtime::state::fixtures::*;
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tower::ServiceExt;

#[test]
fn project_isolation() {
    let root = tempfile::tempdir().unwrap();
    let state = started(
        root.path(),
        &[
            entry(root.path(), "legacy", None),
            entry(root.path(), "a1", Some("a")),
            entry(root.path(), "b1", Some("b")),
            entry(root.path(), "b2", Some("b")),
        ],
        &[project("a"), project("b")],
    );

    let names = |project: &str| -> Vec<String> {
        let sources = project_sources(&state, project).unwrap();
        sources.into_iter().map(|source| source.id).collect()
    };
    assert_eq!(names("default"), ["legacy"]);
    assert_eq!(names("a"), ["a1"]);
    assert_eq!(names("b"), ["b1", "b2"]);
    let legacy: Vec<_> = legacy_nodes(&state).into_iter().map(|n| n.id).collect();
    assert_eq!(legacy, ["legacy"]);

    let detail = source_detail(&state, "b", "b2").unwrap();
    assert_eq!((detail.name.as_str(), detail.kind.as_str()), ("B2", "csv"));
    assert_eq!(detail.project_id, "b");
    assert_eq!(detail.views.len(), 1);
    assert_eq!(detail.views[0].source_id, "b2");
    assert_eq!(
        failure(source_detail(&state, "a", "b2")),
        (404, "Source not found".to_owned())
    );
    for unknown in [
        failure(project_sources(&state, "nope")),
        failure(source_detail(&state, "nope", "a1")),
        failure(project_views(&state, "nope")),
    ] {
        assert_eq!(unknown, (404, "Project not found".to_owned()));
    }

    let views = project_views(&state, "b").unwrap();
    let sources: Vec<_> = views.iter().map(|view| view.source_id.as_str()).collect();
    assert_eq!(sources, ["b1", "b2"]);
    assert!(views.iter().all(|view| view.project_id == "b"));
    let both = datasets(&state, "project_b").unwrap();
    assert_eq!(both.len(), 2);
    assert!(datasets(&state, "project_a").unwrap().is_empty());
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
async fn source_and_view_routes() {
    let root = tempfile::tempdir().unwrap();
    let project = json!({"id": "p", "name": "P", "node_id": "project_p"});
    let app = app(
        root.path(),
        &[
            entry(root.path(), "one", Some("p")),
            entry(root.path(), "two", Some("p")),
        ],
        &[project],
    );

    let (status, sources) = get_json(&app, "/api/projects/p/sources").await;
    assert_eq!(status, 200);
    assert_eq!(
        sources,
        json!([{"id": "one", "name": "ONE"}, {"id": "two", "name": "TWO"}])
    );

    let (status, detail) = get_json(&app, "/api/projects/p/sources/two").await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            detail["id"].as_str(),
            detail["kind"].as_str(),
            detail["project_id"].as_str()
        ),
        (Some("two"), Some("csv"), Some("p"))
    );
    assert_eq!(detail["views"].as_array().unwrap().len(), 1);
    assert_eq!(detail["views"][0]["source_id"], "two");

    let (status, views) = get_json(&app, "/api/projects/p/views").await;
    assert_eq!(status, 200);
    let owners: Vec<_> = views
        .as_array()
        .unwrap()
        .iter()
        .map(|view| view["source_id"].as_str().unwrap())
        .collect();
    assert_eq!(owners, ["one", "two"]);

    for (uri, detail) in [
        ("/api/projects/nope/sources", "Project not found"),
        ("/api/projects/nope/views", "Project not found"),
        ("/api/projects/p/sources/nope", "Source not found"),
    ] {
        let (status, body) = get_json(&app, uri).await;
        assert_eq!((status, body), (404, json!({"detail": detail})), "{uri}");
    }
}

#[tokio::test]
async fn unknown_api_route_is_501() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path(), &[], &[]);

    let (status, body) = get_json(&app, "/api/export").await;
    assert_eq!(
        (status, body),
        (501, json!({"detail": "Not in the desktop build yet"}))
    );
    let (status, _) = send(&app, Method::GET, "/api/nodes/upload", None).await;
    assert_eq!(status, 501);
    let (status, body) = send(
        &app,
        Method::POST,
        "/api/projects/default/sources/attach",
        Some("{}"),
    )
    .await;
    assert_eq!(status, 501);
    assert_eq!(body, json!({"detail": "Not in the desktop build yet"}));
    let (status, _) = get_json(&app, "/elsewhere").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn datasets_route_lists_columns() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path(), &[entry(root.path(), "legacy", None)], &[]);

    let (status, nodes) = get_json(&app, "/api/nodes").await;
    assert_eq!(status, 200);
    assert_eq!(nodes.as_array().unwrap().len(), 1);
    assert_eq!(
        (
            nodes[0]["id"].as_str(),
            nodes[0]["name"].as_str(),
            nodes[0]["kind"].as_str()
        ),
        (Some("legacy"), Some("LEGACY"), Some("csv"))
    );

    let (status, datasets) = get_json(&app, "/api/nodes/legacy/datasets").await;
    assert_eq!(status, 200);
    assert_eq!(
        datasets,
        json!([{
            "id": "WyJtYWluIiwiZGF0YSJd", "name": "data", "schema": "main",
            "type": "VIEW", "columns": ["a", "b"],
        }])
    );

    let (status, body) = get_json(&app, "/api/nodes/nope/datasets").await;
    assert_eq!((status, body), (404, json!({"detail": "Node not found"})));
}
