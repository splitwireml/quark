//! Upload and delete routes.

use std::path::Path;

use axum::extract::multipart::{Field, MultipartError};
use axum::extract::{DefaultBodyLimit, FromRequest, Multipart, Path as UrlPath, Request, State};
use axum::http::StatusCode;
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde_json::Value;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::blocking;
use crate::error::{ApiError, ApiResult};
use crate::registry::DEFAULT_PROJECT_ID;
use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/sources/upload",
            post(upload_to_project).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/api/nodes/upload",
            post(upload_to_default).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/api/projects/{project_id}/sources/{source_id}",
            delete(delete_project_source),
        )
        .route("/api/nodes/{node_id}", delete(delete_node))
}

async fn upload_to_project(
    State(state): State<AppState>,
    UrlPath(project_id): UrlPath<String>,
    request: Request,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let known = state.clone();
    let id = project_id.clone();
    blocking(move || known.project_sources(&id).map(drop)).await?;
    upload(state, Some(project_id), request).await
}

async fn upload_to_default(
    State(state): State<AppState>,
    request: Request,
) -> ApiResult<(StatusCode, Json<Value>)> {
    upload(state, None, request).await
}

/// Saves the multipart field named `file`, then registers it as a source.
async fn upload(
    state: AppState,
    project_id: Option<String>,
    request: Request,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let mut form = Multipart::from_request(request, &state)
        .await
        .map_err(|rejection| ApiError::unprocessable(rejection.body_text()))?;
    while let Some(field) = form.next_field().await.map_err(invalid_form)? {
        if field.name() == Some("file") {
            let node = store(state, project_id, field).await?;
            return Ok((StatusCode::CREATED, Json(node)));
        }
    }
    Err(file_required())
}

async fn store(state: AppState, project_id: Option<String>, field: Field<'_>) -> ApiResult<Value> {
    let name = field
        .file_name()
        .and_then(|name| name.rsplit(['/', '\\']).next())
        .ok_or_else(file_required)?
        .to_owned();
    let ext = supported_extension(&name)?;
    let node_id = Uuid::new_v4().simple().to_string();
    let path = state.upload_path(&node_id, &ext);
    if let Err(error) = write_field(field, &path).await {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error);
    }
    blocking(move || state.register_upload(project_id.as_deref(), &node_id, &name, &path)).await
}

/// Streams the field into a new file chunk by chunk, so memory stays flat.
async fn write_field(mut field: Field<'_>, path: &Path) -> ApiResult<()> {
    let mut file = File::create_new(path).await.map_err(unsaved)?;
    while let Some(chunk) = field.chunk().await.map_err(invalid_form)? {
        file.write_all(&chunk).await.map_err(unsaved)?;
    }
    file.flush().await.map_err(unsaved)
}

/// The lowercase extension with its dot, if the desktop build can read that kind of file.
fn supported_extension(name: &str) -> ApiResult<String> {
    let ext = Path::new(name)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    match ext.as_str() {
        ".csv" | ".tsv" | ".parquet" | ".json" | ".ndjson" | ".jsonl" | ".duckdb" | ".db" => {
            Ok(ext)
        }
        ".xlsx" => Err(ApiError::not_implemented("Not in the desktop build yet")),
        _ => Err(ApiError::bad_request("Unsupported file type")),
    }
}

fn file_required() -> ApiError {
    ApiError::unprocessable("A file field is required")
}

fn invalid_form(error: MultipartError) -> ApiError {
    ApiError::unprocessable(error.body_text())
}

fn unsaved(error: std::io::Error) -> ApiError {
    tracing::error!(%error, "could not save an upload");
    ApiError::internal("Internal error")
}

async fn delete_project_source(
    State(state): State<AppState>,
    UrlPath((project_id, source_id)): UrlPath<(String, String)>,
) -> ApiResult<StatusCode> {
    blocking(move || state.delete_source(&project_id, &source_id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_node(
    State(state): State<AppState>,
    UrlPath(node_id): UrlPath<String>,
) -> ApiResult<StatusCode> {
    blocking(move || state.delete_source(DEFAULT_PROJECT_ID, &node_id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::super::router;
    use super::*;
    use crate::engine::Dirs;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::fs;
    use std::path::Path;
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
            body.extend_from_slice(
                format!("Content-Disposition: {disposition}\r\n\r\n").as_bytes(),
            );
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
        let (status, body) =
            upload(&app, "/api/projects/p/sources/upload", "Book.XLSX", b"x").await;
        assert_eq!(
            (status, body),
            (501, json!({"detail": "Not in the desktop build yet"}))
        );
        let (status, body) =
            upload(&app, "/api/nodes/upload", "broken.parquet", b"not parquet").await;
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
        let (_, member) =
            upload(&app, "/api/projects/p/sources/upload", "two.csv", b"a\n2\n").await;
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
        let (status, body) =
            send(&app, Method::DELETE, &format!("/api/nodes/{legacy}"), None).await;
        assert_eq!((status, body), (204, Value::Null));
        let (status, body) =
            send(&app, Method::DELETE, &format!("/api/nodes/{legacy}"), None).await;
        assert_eq!((status, body), (404, json!({"detail": "Node not found"})));

        assert!(uploaded_files(&state).is_empty());
        let (_, nodes) = send(&app, Method::GET, "/api/nodes", None).await;
        assert_eq!(nodes, json!([]));
    }
}
