//! Tests for staging workbook uploads, confirming a sheet selection and discarding a stage.

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use rust_xlsxwriter::Workbook;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::http::router;
use crate::runtime::AppState;
use crate::runtime::registry::load_registry;
use crate::runtime::state::fixtures::*;

const BOUNDARY: &str = "workbook-test-boundary";

fn app(root: &Path) -> (Router, AppState) {
    let state = started(root, &[], &[project("p")]);
    (router(state.clone()), state)
}

fn workbook_bytes(root: &Path) -> Vec<u8> {
    let path = root.join("sheets.xlsx");
    let mut workbook = Workbook::new();
    for (name, value) in [("People", "Ada"), ("O'Reilly", "SQL")] {
        let sheet = workbook.add_worksheet();
        sheet.set_name(name).unwrap();
        sheet.write_string(0, 0, "name").unwrap();
        sheet.write_string(1, 0, value).unwrap();
    }
    workbook.save(&path).unwrap();
    fs::read(path).unwrap()
}

async fn send(app: &Router, method: Method, uri: &str, body: Option<Value>) -> (u16, Value) {
    let request = Request::builder().method(method).uri(uri);
    let (request, body) = match body {
        Some(json) => (
            request.header("content-type", "application/json"),
            Body::from(json.to_string()),
        ),
        None => (request, Body::empty()),
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

async fn upload(app: &Router, uri: &str, bytes: &[u8]) -> (u16, Value) {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sheets.xlsx\"\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn uploads(state: &AppState) -> usize {
    fs::read_dir(state.0.dirs.uploads()).unwrap().count()
}

#[tokio::test]
async fn staging_lists_sheets_in_workbook_order() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let bytes = workbook_bytes(root.path());

    let (status, legacy) = upload(&app, "/api/nodes/upload", &bytes).await;
    let (project_status, scoped) = upload(&app, "/api/projects/p/sources/upload", &bytes).await;

    assert_eq!((status, project_status), (201, 201));
    let sheets = json!(["People", "O'Reilly"]);
    let id = legacy["id"].as_str().unwrap();
    assert_eq!(
        legacy,
        json!({"id": id, "name": "sheets.xlsx", "kind": "workbook", "sheets": sheets})
    );
    assert_eq!(scoped["project_id"], "p");
    assert_eq!(scoped["sheets"], sheets);
    assert!(state.catalog().sources.is_empty());
    assert_eq!(uploads(&state), 2);
}

#[tokio::test]
async fn confirm_validates_the_selection() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let (_, staged) = upload(&app, "/api/nodes/upload", &workbook_bytes(root.path())).await;
    let id = staged["id"].as_str().unwrap();
    let confirm = format!("/api/nodes/upload/{id}/confirm");

    for sheets in [json!([]), json!(["People", "People"]), json!(["missing"])] {
        let (status, body) = send(
            &app,
            Method::POST,
            &confirm,
            Some(json!({"sheets": sheets})),
        )
        .await;
        assert_eq!(
            (status, body),
            (
                422,
                json!({"detail": "Select one or more unique workbook sheets"})
            )
        );
    }
    let (status, node) = send(
        &app,
        Method::POST,
        &confirm,
        Some(json!({"sheets": ["O'Reilly"]})),
    )
    .await;

    assert_eq!(status, 200);
    let source = state.upload_path(id, ".xlsx");
    assert_eq!(
        node,
        json!({"id": id, "name": "sheets.xlsx", "kind": "upload", "source": source.to_string_lossy()})
    );
    let saved = load_registry(&state.0.dirs.registry_file());
    assert_eq!(saved[0]["sheets"], json!(["O'Reilly"]));
    assert!(saved[0].get("dataset_name").is_none());
    let (again, _) = send(
        &app,
        Method::POST,
        &confirm,
        Some(json!({"sheets": ["People"]})),
    )
    .await;
    assert_eq!(again, 404);
}

#[tokio::test]
async fn confirm_from_another_project_is_404() {
    let root = tempfile::tempdir().unwrap();
    let (app, _) = app(root.path());
    let bytes = workbook_bytes(root.path());
    let (_, legacy) = upload(&app, "/api/nodes/upload", &bytes).await;
    let (_, scoped) = upload(&app, "/api/projects/p/sources/upload", &bytes).await;
    let body = Some(json!({"sheets": ["People"]}));
    let missing = json!({"detail": "Workbook upload not found"});

    for (uri, id) in [
        ("/api/projects/p/sources/upload", &legacy["id"]),
        ("/api/nodes/upload", &scoped["id"]),
    ] {
        let uri = format!("{uri}/{}", id.as_str().unwrap());
        let (status, detail) =
            send(&app, Method::POST, &format!("{uri}/confirm"), body.clone()).await;
        assert_eq!((status, detail), (404, missing.clone()), "{uri}");
        assert_eq!(
            send(&app, Method::DELETE, &uri, None).await,
            (404, missing.clone())
        );
    }
    let uri = "/api/projects/nope/sources/upload/x";
    let (status, detail) = send(&app, Method::DELETE, uri, None).await;
    assert_eq!(
        (status, detail),
        (404, json!({"detail": "Project not found"}))
    );
    let (status, node) = send(
        &app,
        Method::POST,
        &format!(
            "/api/projects/p/sources/upload/{}/confirm",
            scoped["id"].as_str().unwrap()
        ),
        body,
    )
    .await;
    assert_eq!((status, node["project_id"].as_str()), (200, Some("p")));
}

#[tokio::test]
async fn discard_deletes_the_file() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let (_, staged) = upload(&app, "/api/nodes/upload", &workbook_bytes(root.path())).await;
    let uri = format!("/api/nodes/upload/{}", staged["id"].as_str().unwrap());

    let (status, _) = send(&app, Method::DELETE, &uri, None).await;

    assert_eq!(status, 204);
    assert_eq!(uploads(&state), 0);
    assert_eq!(send(&app, Method::DELETE, &uri, None).await.0, 404);
}

#[tokio::test]
async fn unreadable_workbook_is_400() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());

    let (status, body) = upload(&app, "/api/nodes/upload", b"not a zip").await;

    assert_eq!(status, 400);
    assert!(
        body["detail"]
            .as_str()
            .unwrap()
            .starts_with("Could not open source: ")
    );
    assert_eq!(uploads(&state), 0);
}
