//! The join preview route.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use super::model::{JoinPreview, JoinWorkspaceRequest};
use super::service;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, blocking};
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/join-workspaces", post(preview_join))
}

async fn preview_join(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<JoinWorkspaceRequest>,
) -> ApiResult<Json<JoinPreview>> {
    service::validate(&request)?;
    blocking(move || service::preview(&state, &request))
        .await
        .map(Json)
}
