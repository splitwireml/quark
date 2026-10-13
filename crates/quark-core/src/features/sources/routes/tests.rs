//! Tests for the upload and delete routes, including the body-limit and clean-up guarantees.

use crate::http::router;
use crate::runtime::AppState;
use crate::runtime::dirs::Dirs;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{Method, Request};
use http_body_util::{BodyExt, Channel};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const BOUNDARY: &str = "quark-test-boundary";

fn app(root: &Path) -> (Router, AppState) {
    let dirs = Dirs {
        data: root.join("data"),
        cache: root.join("cache"),
    };
    fs::create_dir_all(&dirs.data).unwrap();
    fs::write(
        dirs.projects_file(),
        r#"[{"id":"p","name":"P","node_id":"project_p"}]"#,
    )
    .unwrap();
    let state = AppState::load(dirs).unwrap();
    (router(state.clone()), state)
}

/// A multipart body with one part per `(field, file name, content)`.
fn form(parts: &[(&str, Option<&str>, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (field, file, content) in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        let disposition = match file {
            Some(file) => format!("form-data; name=\"{field}\"; filename=\"{file}\""),
            None => format!("form-data; name=\"{field}\""),
        };
        body.extend_from_slice(format!("Content-Disposition: {disposition}\r\n\r\n").as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn send(app: &Router, method: Method, uri: &str, body: Option<Vec<u8>>) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(bytes) => {
            request = request.header(
                "content-type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
            Body::from(bytes)
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

async fn upload(app: &Router, uri: &str, file_name: &str, content: &[u8]) -> (u16, Value) {
    let body = form(&[("file", Some(file_name), content)]);
    send(app, Method::POST, uri, Some(body)).await
}

fn uploaded_files(state: &AppState) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(state.0.dirs.uploads())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn upload_over_two_megabytes_streams() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let mut csv = String::from("a,b\n");
    while csv.len() < 3 * 1024 * 1024 {
        csv.push_str("12345,some text value\n");
    }
    let rows = csv.lines().count() - 1;

    let (status, node) = upload(
        &app,
        "/api/nodes/upload",
        "reports\\Big Sheet.CSV",
        csv.as_bytes(),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(node["name"], "Big Sheet.CSV");
    assert_eq!(node["kind"], "upload");
    let id = node["id"].as_str().unwrap();
    assert_eq!(id.len(), 32);
    let saved = state.upload_path(id, ".csv");
    assert_eq!(fs::metadata(&saved).unwrap().len(), csv.len() as u64);
    assert_eq!(node["source"], saved.to_string_lossy().as_ref());

    let (status, nodes) = send(&app, Method::GET, "/api/nodes", None).await;
    assert_eq!(status, 200);
    assert_eq!(nodes[0]["id"], id);
    let engine = state.engine_for_node(id).unwrap();
    let count: i64 = engine
        .lock()
        .conn
        .query_row("SELECT count(*) FROM main.big_sheet", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, rows as i64);

    let (status, node) = upload(
        &app,
        "/api/projects/p/sources/upload",
        "small.tsv",
        b"a\tb\n1\t2\n",
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(node["project_id"], "p");
    let (_, sources) = send(&app, Method::GET, "/api/projects/p/sources", None).await;
    assert_eq!(sources[0]["id"], node["id"]);
}

#[tokio::test]
async fn aborted_upload_leaves_no_file() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let mut head = form(&[("file", Some("cut.csv"), &[b'x'; 64 * 1024])]);
    head.truncate(head.len() - 64);
    let (mut sender, body) = Channel::<Bytes, std::io::Error>::new(1);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/nodes/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::new(body))
        .unwrap();
    let uploads = state.0.dirs.uploads();
    let client = tokio::spawn(async move {
        sender.send_data(Bytes::from(head)).await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while fs::read_dir(&uploads).unwrap().next().is_none() {
            assert!(Instant::now() < deadline, "the partial file never appeared");
            tokio::task::yield_now().await;
        }
        sender.abort(std::io::Error::other("connection lost"));
    });

    let response = app.oneshot(request).await.unwrap();
    client.await.unwrap();

    assert!(response.status().is_client_error());
    assert!(uploaded_files(&state).is_empty());
}

#[tokio::test]
async fn json_routes_keep_the_two_megabyte_limit() {
    let root = tempfile::tempdir().unwrap();
    let (app, _) = app(root.path());
    let padding = " ".repeat(2 * 1024 * 1024 + 1);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/nodes/legacy/sql")
        .header("content-type", "application/json")
        .body(Body::from(format!("{{\"sql\":\"SELECT 1\"{padding}}}")))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status().as_u16(), 413);
}

#[tokio::test]
async fn unsupported_and_xlsx_extensions() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());

    for name in ["notes.txt", "noextension", ".csv", "archive.csv.zip"] {
        let (status, body) = upload(&app, "/api/nodes/upload", name, b"a\n1\n").await;
        assert_eq!(
            (status, body),
            (400, json!({"detail": "Unsupported file type"})),
            "{name}"
        );
    }
    let (status, body) = upload(&app, "/api/projects/p/sources/upload", "Book.XLSX", b"x").await;
    assert_eq!(
        (status, body),
        (501, json!({"detail": "Not in the desktop build yet"}))
    );
    let (status, body) = upload(&app, "/api/nodes/upload", "broken.parquet", b"not parquet").await;
    assert_eq!(status, 400);
    assert!(
        body["detail"]
            .as_str()
            .unwrap()
            .starts_with("Could not open source: ")
    );
    assert!(uploaded_files(&state).is_empty());
}

#[tokio::test]
async fn missing_file_field_is_422() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());

    let cases = [
        Some(form(&[("other", Some("a.csv"), b"a\n1\n")])),
        Some(form(&[("file", None, b"a\n1\n")])),
        Some(form(&[])),
        None,
    ];
    for body in cases {
        let (status, detail) = send(&app, Method::POST, "/api/nodes/upload", body).await;
        assert_eq!(status, 422);
        assert!(detail["detail"].is_string());
    }
    let (status, body) = upload(
        &app,
        "/api/projects/nope/sources/upload",
        "a.csv",
        b"a\n1\n",
    )
    .await;
    assert_eq!(
        (status, body),
        (404, json!({"detail": "Project not found"}))
    );
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/projects/nope/sources/upload",
        None,
    )
    .await;
    assert_eq!(status, 404);
    assert!(uploaded_files(&state).is_empty());
}

#[tokio::test]
async fn delete_routes() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let (_, legacy) = upload(&app, "/api/nodes/upload", "one.csv", b"a\n1\n").await;
    let (_, member) = upload(&app, "/api/projects/p/sources/upload", "two.csv", b"a\n2\n").await;
    let (legacy, member) = (
        legacy["id"].as_str().unwrap(),
        member["id"].as_str().unwrap(),
    );
    assert_eq!(uploaded_files(&state).len(), 2);

    let (status, body) = send(
        &app,
        Method::DELETE,
        &format!("/api/projects/p/sources/{legacy}"),
        None,
    )
    .await;
    assert_eq!((status, body), (404, json!({"detail": "Node not found"})));
    let (status, body) = send(&app, Method::DELETE, "/api/projects/nope/sources/x", None).await;
    assert_eq!(
        (status, body),
        (404, json!({"detail": "Project not found"}))
    );

    let (status, body) = send(
        &app,
        Method::DELETE,
        &format!("/api/projects/p/sources/{member}"),
        None,
    )
    .await;
    assert_eq!((status, body), (204, Value::Null));
    let (status, body) = send(&app, Method::DELETE, &format!("/api/nodes/{legacy}"), None).await;
    assert_eq!((status, body), (204, Value::Null));
    let (status, body) = send(&app, Method::DELETE, &format!("/api/nodes/{legacy}"), None).await;
    assert_eq!((status, body), (404, json!({"detail": "Node not found"})));

    assert!(uploaded_files(&state).is_empty());
    let (_, nodes) = send(&app, Method::GET, "/api/nodes", None).await;
    assert_eq!(nodes, json!([]));
}
