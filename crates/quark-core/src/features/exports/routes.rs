//! The export route: checks each sheet, writes the file and streams it back as a download.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::Response;
use axum::routing::post;
use tokio::fs::File;
use tokio::io::{AsyncRead, ReadBuf};
use tokio_util::io::ReaderStream;

use super::model::{ExportFormat, ExportRequest};
use super::names::{content_disposition, export_filename, extension, media_type};
use super::service::{
    ExportFile, PreparedSheet, prepare_sheet, require_single_sheet, write_export,
};
use crate::error::{ApiError, ApiResult};
use crate::http::deps::{ApiJson, Reader, blocking};
use crate::runtime::AppState;
use crate::runtime::engine::Engine;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/exports", post(export))
}

/// An export file being read: the file handle drops before the guard, so Windows can delete it.
struct Download {
    file: File,
    _export: ExportFile,
}

impl AsyncRead for Download {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

/// Each sheet's node and SQL are checked in turn, as Python does, before anything is written.
/// Writing runs to the end even if the client leaves: the file is deleted when its body drops.
async fn export(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ExportRequest>,
) -> ApiResult<Response> {
    request.validate()?;
    require_single_sheet(&request)?;
    let mut prepared: Vec<(Arc<Engine>, PreparedSheet)> = Vec::with_capacity(request.sheets.len());
    for sheet in request.sheets {
        let lookup = state.clone();
        let node_id = sheet.node_id.clone();
        let engine = blocking(move || lookup.engine_for_node(&node_id)).await?;
        let checked = Reader::new(Arc::clone(&engine))
            .run_to_end(move |_, conn| prepare_sheet(conn, &sheet))
            .await?;
        prepared.push((engine, checked));
    }
    let (format, layout, dir) = (request.format, request.json_layout, state.0.dirs.exports());
    let export = blocking(move || write_export(format, layout, &prepared, &dir)).await?;
    download(export, format, request.filename.as_deref()).await
}

async fn download(
    export: ExportFile,
    format: ExportFormat,
    filename: Option<&str>,
) -> ApiResult<Response> {
    let unreadable = |error: io::Error| {
        tracing::error!(%error, "could not read an export");
        ApiError::internal("Internal error")
    };
    let file = File::open(export.path()).await.map_err(unreadable)?;
    let length = file.metadata().await.map_err(unreadable)?.len();
    let disposition = content_disposition(&export_filename(filename, extension(format)));
    let body = Body::from_stream(ReaderStream::new(Download {
        file,
        _export: export,
    }));
    Response::builder()
        .header(CONTENT_TYPE, media_type(format))
        .header(CONTENT_DISPOSITION, disposition)
        .header(CONTENT_LENGTH, length)
        .body(body)
        .map_err(|_| ApiError::internal("Internal error"))
}
