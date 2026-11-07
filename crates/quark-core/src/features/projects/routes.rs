//! Routes for listing and creating projects.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use super::model::{ProjectCreate, ProjectSummary};
use super::service;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, blocking};
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/projects", get(list_projects).post(create_project))
}

async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<ProjectSummary>>> {
    blocking(move || Ok(service::list_projects(&state)))
        .await
        .map(Json)
}

async fn create_project(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ProjectCreate>,
) -> ApiResult<(StatusCode, Json<ProjectSummary>)> {
    let project = blocking(move || service::create_project(&state, &request.name)).await?;
    Ok((StatusCode::CREATED, Json(project)))
}
