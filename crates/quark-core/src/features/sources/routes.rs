//! Routes for listing sources, views and datasets, uploading files and deleting sources.

use axum::body::Bytes;
use axum::extract::multipart::{Field, MultipartError};
use axum::extract::{DefaultBodyLimit, FromRequest, Multipart, Path, Request, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::Value;

use super::model::{DatasetEntry, LegacyNode, SourceDetail, SourceSummary};
use super::service;
use super::uploads::{ChunkSource, register_upload, store};
use crate::error::{ApiError, ApiResult};
use crate::http::deps::blocking;
use crate::runtime::AppState;
use crate::runtime::mount::ViewInfo;
use crate::runtime::registry::DEFAULT_PROJECT_ID;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/sources", get(list_sources))
        .route(
            "/api/projects/{project_id}/sources/upload",
            post(upload_to_project).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/api/projects/{project_id}/sources/{source_id}",
            get(get_source).delete(delete_project_source),
        )
        .route("/api/projects/{project_id}/views", get(list_views))
        .route("/api/nodes", get(list_nodes))
        .route(
            "/api/nodes/upload",
            post(upload_to_default).layer(DefaultBodyLimit::disable()),
        )
        .route("/api/nodes/{node_id}", delete(delete_node))
        .route("/api/nodes/{node_id}/datasets", get(list_datasets))
}

async fn list_sources(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<SourceSummary>>> {
    blocking(move || service::project_sources(&state, &project_id))
        .await
        .map(Json)
}

async fn get_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> ApiResult<Json<SourceDetail>> {
    blocking(move || service::source_detail(&state, &project_id, &source_id))
        .await
        .map(Json)
}

async fn list_views(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<ViewInfo>>> {
    blocking(move || service::project_views(&state, &project_id))
        .await
        .map(Json)
}

async fn list_nodes(State(state): State<AppState>) -> ApiResult<Json<Vec<LegacyNode>>> {
    blocking(move || Ok(service::legacy_nodes(&state)))
        .await
        .map(Json)
}

async fn list_datasets(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> ApiResult<Json<Vec<DatasetEntry>>> {
    blocking(move || service::datasets(&state, &node_id))
        .await
        .map(Json)
}

async fn upload_to_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    request: Request,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let known = state.clone();
    let id = project_id.clone();
    blocking(move || service::project_sources(&known, &id).map(drop)).await?;
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
        if field.name() != Some("file") {
            continue;
        }
        let client_name = field.file_name().ok_or_else(file_required)?.to_owned();
        let saved = store(&state, &client_name, &mut FormField(field)).await?;
        let node = blocking(move || {
            register_upload(
                &state,
                project_id.as_deref(),
                &saved.node_id,
                &saved.name,
                &saved.path,
            )
        })
        .await?;
        return Ok((StatusCode::CREATED, Json(node)));
    }
    Err(file_required())
}

/// A multipart field read chunk by chunk.
struct FormField<'a>(Field<'a>);

impl ChunkSource for FormField<'_> {
    type Chunk = Bytes;

    async fn next_chunk(&mut self) -> ApiResult<Option<Bytes>> {
        self.0.chunk().await.map_err(invalid_form)
    }
}

fn file_required() -> ApiError {
    ApiError::unprocessable("A file field is required")
}

fn invalid_form(error: MultipartError) -> ApiError {
    ApiError::unprocessable(error.body_text())
}

async fn delete_project_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    blocking(move || state.delete_source(&project_id, &source_id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> ApiResult<StatusCode> {
    blocking(move || state.delete_source(DEFAULT_PROJECT_ID, &node_id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
