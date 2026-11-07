//! Routes that confirm or discard a staged workbook, for a project and for the default one.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde_json::Value;

use super::model::WorkbookConfirm;
use super::service;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, blocking};
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/sources/upload/{stage_id}/confirm",
            post(confirm_in_project),
        )
        .route(
            "/api/projects/{project_id}/sources/upload/{stage_id}",
            delete(discard_in_project),
        )
        .route(
            "/api/nodes/upload/{stage_id}/confirm",
            post(confirm_in_default),
        )
        .route("/api/nodes/upload/{stage_id}", delete(discard_in_default))
}

async fn confirm_in_project(
    State(state): State<AppState>,
    Path((project_id, stage_id)): Path<(String, String)>,
    ApiJson(request): ApiJson<WorkbookConfirm>,
) -> ApiResult<Json<Value>> {
    confirm(state, Some(project_id), stage_id, request).await
}

async fn confirm_in_default(
    State(state): State<AppState>,
    Path(stage_id): Path<String>,
    ApiJson(request): ApiJson<WorkbookConfirm>,
) -> ApiResult<Json<Value>> {
    confirm(state, None, stage_id, request).await
}

async fn confirm(
    state: AppState,
    project_id: Option<String>,
    stage_id: String,
    request: WorkbookConfirm,
) -> ApiResult<Json<Value>> {
    blocking(move || service::confirm_workbook(&state, project_id.as_deref(), &stage_id, &request))
        .await
        .map(Json)
}

async fn discard_in_project(
    State(state): State<AppState>,
    Path((project_id, stage_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    discard(state, Some(project_id), stage_id).await
}

async fn discard_in_default(
    State(state): State<AppState>,
    Path(stage_id): Path<String>,
) -> ApiResult<StatusCode> {
    discard(state, None, stage_id).await
}

async fn discard(
    state: AppState,
    project_id: Option<String>,
    stage_id: String,
) -> ApiResult<StatusCode> {
    blocking(move || service::discard_workbook(&state, project_id.as_deref(), &stage_id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
