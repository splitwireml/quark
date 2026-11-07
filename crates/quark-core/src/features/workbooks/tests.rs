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

/// A zip written entry by entry without compression: `rust_xlsxwriter` always adds a sheet, so a
/// workbook with none has to be assembled by hand.
fn stored_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }
    let (mut archive, mut directory) = (Vec::new(), Vec::new());
    for (name, text) in entries {
        let (size, crc) = (text.len() as u32, crc32(text.as_bytes()));
        let offset = archive.len() as u32;
        let common = [
            20u16.to_le_bytes().as_slice(),
            &[0; 8],
            &crc.to_le_bytes(),
            &size.to_le_bytes(),
            &size.to_le_bytes(),
            &(name.len() as u16).to_le_bytes(),
            &[0; 2],
        ]
        .concat();
        archive.extend_from_slice(&[b'P', b'K', 3, 4]);
        archive.extend_from_slice(&common);
        archive.extend_from_slice(name.as_bytes());
        archive.extend_from_slice(text.as_bytes());
        directory.extend_from_slice(&[b'P', b'K', 1, 2, 20, 0]);
        directory.extend_from_slice(&common);
        directory.extend_from_slice(&[0; 10]);
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let (count, start) = (entries.len() as u16, archive.len() as u32);
    archive.extend_from_slice(&directory);
    archive.extend_from_slice(&[b'P', b'K', 5, 6, 0, 0, 0, 0]);
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    archive.extend_from_slice(&start.to_le_bytes());
    archive.extend_from_slice(&[0; 2]);
    archive
}

fn workbook_without_sheets() -> Vec<u8> {
    const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    const DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    stored_zip(&[
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>",
        ),
        (
            "_rels/.rels",
            &format!(
                "<Relationships xmlns=\"{RELS}\"><Relationship Id=\"rId1\" Type=\"{DOC}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
            ),
        ),
        (
            "xl/workbook.xml",
            &format!("<workbook xmlns=\"{MAIN}\" xmlns:r=\"{DOC}\"><sheets/></workbook>"),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            &format!("<Relationships xmlns=\"{RELS}\"/>"),
        ),
    ])
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

#[tokio::test]
async fn workbook_without_sheets_is_400_and_leaves_no_file() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());

    let (status, body) = upload(&app, "/api/nodes/upload", &workbook_without_sheets()).await;

    assert_eq!(status, 400);
    assert_eq!(
        body["detail"],
        "Could not open source: Workbook has no sheets"
    );
    assert_eq!(uploads(&state), 0);
}

#[tokio::test]
async fn importing_a_workbook_never_loads_the_excel_extension() {
    let root = tempfile::tempdir().unwrap();
    let (app, state) = app(root.path());
    let (_, staged) = upload(&app, "/api/nodes/upload", &workbook_bytes(root.path())).await;
    let id = staged["id"].as_str().unwrap();
    let confirm = format!("/api/nodes/upload/{id}/confirm");
    let (status, _) = send(
        &app,
        Method::POST,
        &confirm,
        Some(json!({"sheets": ["People", "O'Reilly"]})),
    )
    .await;
    assert_eq!(status, 200);

    let engine = state.engine_for_node(id).unwrap();
    let primary = engine.primary();
    let count = |sql: &str| -> i64 { primary.conn.query_row(sql, [], |row| row.get(0)).unwrap() };

    assert!(count("SELECT count(*) FROM information_schema.tables") >= 2);
    assert_eq!(
        count("SELECT count(*) FROM duckdb_functions() WHERE function_name = 'read_xlsx'"),
        0,
        "the excel extension's reader is registered on the node's engine"
    );
}
