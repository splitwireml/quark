//! Tests for listing sources, views and datasets, in the services and over HTTP.

use super::attach::expand_path;
use super::service::{datasets, legacy_nodes, project_sources, project_views, source_detail};
use crate::http::router;
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

/// A DuckDB file holding one table, `t(n)`, with the rows 1 and 2.
fn database_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let conn = duckdb::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE t(n INTEGER); INSERT INTO t VALUES (1), (2);")
        .unwrap();
    path
}

fn attach_body(path: &std::path::Path) -> String {
    json!({"path": path.to_string_lossy()}).to_string()
}

#[tokio::test]
async fn attach_rejects_other_suffixes_before_missing_files() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path(), &[], &[project("p")]);
    let missing = |name: &str| attach_body(&root.path().join(name));

    for uri in ["/api/nodes/attach", "/api/projects/p/sources/attach"] {
        for (body, expected) in [
            (
                missing("gone.csv"),
                (400, "Only .duckdb and .db paths can be attached"),
            ),
            (missing("gone.DB"), (404, "Database not found")),
            (missing("gone.duckdb"), (404, "Database not found")),
            (
                attach_body(root.path()),
                (400, "Only .duckdb and .db paths can be attached"),
            ),
        ] {
            let (status, reply) = send(&app, Method::POST, uri, Some(&body)).await;
            assert_eq!(
                (status, reply),
                (expected.0, json!({"detail": expected.1})),
                "{uri}"
            );
        }
    }
    let directory = root.path().join("folder.db");
    fs::create_dir(&directory).unwrap();
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/nodes/attach",
        Some(&attach_body(&directory)),
    )
    .await;
    assert_eq!(status, 404);

    let body = missing("gone.csv");
    let (status, reply) = send(
        &app,
        Method::POST,
        "/api/projects/nope/sources/attach",
        Some(&body),
    )
    .await;
    assert_eq!(
        (status, reply),
        (404, json!({"detail": "Project not found"}))
    );
    for body in ["{}", r#"{"path": "x.db", "extra": 1}"#] {
        let (status, _) = send(&app, Method::POST, "/api/nodes/attach", Some(body)).await;
        assert_eq!(status, 422, "{body}");
    }
}

#[tokio::test]
async fn attached_database_is_read_only() {
    let root = tempfile::tempdir().unwrap();
    let file = database_file(root.path(), "data.duckdb");
    let resolved = expand_path(&file.to_string_lossy())
        .to_string_lossy()
        .into_owned();
    let app = app(root.path(), &[], &[project("p")]);

    let (status, node) = send(
        &app,
        Method::POST,
        "/api/nodes/attach",
        Some(&attach_body(&file)),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(node["kind"], "attached");
    assert_eq!(node["name"], "data.duckdb");
    assert_eq!(node["source"], resolved.as_ref());
    assert_eq!(node["id"].as_str().unwrap().len(), 32);

    let (status, scoped) = send(
        &app,
        Method::POST,
        "/api/projects/p/sources/attach",
        Some(&attach_body(&file)),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(scoped["project_id"], "p");
    assert!(scoped.get("source").is_none());

    let id = node["id"].as_str().unwrap();
    let (status, datasets) = get_json(&app, &format!("/api/nodes/{id}/datasets")).await;
    assert_eq!(status, 200);
    assert_eq!(datasets[0]["name"], "t");
    assert_eq!(datasets[0]["columns"], json!(["n"]));
    let (status, nodes) = get_json(&app, "/api/nodes").await;
    assert_eq!((status, nodes.as_array().unwrap().len()), (200, 1));
}

#[tokio::test]
async fn deleting_an_attachment_keeps_the_file() {
    let root = tempfile::tempdir().unwrap();
    let file = database_file(root.path(), "keep.db");
    let app = app(root.path(), &[], &[project("p")]);

    let (_, node) = send(
        &app,
        Method::POST,
        "/api/projects/p/sources/attach",
        Some(&attach_body(&file)),
    )
    .await;
    let uri = format!("/api/projects/p/sources/{}", node["id"].as_str().unwrap());
    let (status, _) = send(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, 204);
    assert!(file.is_file());
    let (_, sources) = get_json(&app, "/api/projects/p/sources").await;
    assert_eq!(sources, json!([]));
}

#[tokio::test]
async fn source_path_is_absolute() {
    let root = tempfile::tempdir().unwrap();
    let file = database_file(root.path(), "where.db");
    let resolved = expand_path(&file.to_string_lossy())
        .to_string_lossy()
        .into_owned();
    let app = app(
        root.path(),
        &[entry(root.path(), "one", Some("p"))],
        &[project("p")],
    );

    let (_, node) = send(
        &app,
        Method::POST,
        "/api/projects/p/sources/attach",
        Some(&attach_body(&file)),
    )
    .await;
    let uri = format!(
        "/api/projects/p/sources/{}/path",
        node["id"].as_str().unwrap()
    );
    let (status, reply) = get_json(&app, &uri).await;
    assert_eq!((status, reply), (200, json!({"path": resolved})));
    let (status, reply) = get_json(&app, "/api/projects/p/sources/one/path").await;
    assert_eq!(status, 200);
    assert!(std::path::Path::new(reply["path"].as_str().unwrap()).is_absolute());

    for (uri, detail) in [
        ("/api/projects/nope/sources/one/path", "Project not found"),
        ("/api/projects/p/sources/nope/path", "Source not found"),
    ] {
        let (status, reply) = get_json(&app, uri).await;
        assert_eq!((status, reply), (404, json!({"detail": detail})), "{uri}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn attaching_through_a_symlink_reports_the_real_path() {
    let root = tempfile::tempdir().unwrap();
    let real = fs::canonicalize(root.path()).unwrap().join("real");
    fs::create_dir(&real).unwrap();
    let file = database_file(&real, "x.duckdb");
    let link = real.with_file_name("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let app = app(root.path(), &[], &[project("p")]);
    let body = attach_body(&link.join("x.duckdb"));

    let (status, node) = send(&app, Method::POST, "/api/nodes/attach", Some(&body)).await;
    assert_eq!(status, 201);
    assert_eq!(node["source"], file.to_string_lossy().as_ref());
    assert_eq!(node["name"], "x.duckdb");
    let uri = "/api/projects/p/sources/attach";
    let (_, scoped) = send(&app, Method::POST, uri, Some(&body)).await;
    let uri = format!(
        "/api/projects/p/sources/{}/path",
        scoped["id"].as_str().unwrap()
    );
    let (_, reply) = get_json(&app, &uri).await;
    assert_eq!(reply, json!({"path": file.to_string_lossy()}));
}
