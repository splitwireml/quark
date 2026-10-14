//! Request extractors and helpers every route draws on: blocking work, JSON bodies, the output format, the project and the engine.

use axum::extract::{FromRequest, FromRequestParts, RawPathParams, Request};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use duckdb::Connection;
use serde::de::DeserializeOwned;
use std::convert::Infallible;
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::features::pages::model::Format;
use crate::runtime::AppState;
use crate::runtime::cancel::{CancelGuard, Cancelled};
use crate::runtime::engine::Engine;
use crate::runtime::registry::ProjectRecord;
use crate::shared::encode::arrow::ARROW_MEDIA_TYPE;

/// Runs `work` off the async runtime; a task that panics or is cancelled becomes a 500.
pub async fn blocking<T, F>(work: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> ApiResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        tracing::error!(%error, "blocking task failed");
        ApiError::internal("Internal error")
    })?
}

/// A JSON body whose rejections (syntax, content type, schema) are 422s; an over-limit body stays a 413.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(value)| Self(value))
            .map_err(|rejection| match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => {
                    ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, rejection.body_text())
                }
                _ => ApiError::unprocessable(rejection.body_text()),
            })
    }
}

/// Arrow when any `Accept` value names the Arrow stream type, otherwise JSON.
impl<S: Send + Sync> FromRequestParts<S> for Format {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        let wants_arrow = parts.headers.get_all(header::ACCEPT).iter().any(|value| {
            value
                .to_str()
                .is_ok_and(|accept| accept.contains(ARROW_MEDIA_TYPE))
        });
        Ok(if wants_arrow { Self::Arrow } else { Self::Json })
    }
}

/// The project named by the route's `{project_id}`, or 404 `Project not found`.
pub struct Project(pub ProjectRecord);

impl FromRequestParts<AppState> for Project {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let project_id = path_param(parts, state, "project_id").await?;
        let state = state.clone();
        blocking(move || state.catalog().project(&project_id).cloned())
            .await
            .map(Self)
    }
}

/// The engine behind the route's `{node_id}`, opened on first use.
pub struct EngineFor(pub Arc<Engine>);

impl FromRequestParts<AppState> for EngineFor {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let node_id = path_param(parts, state, "node_id").await?;
        let state = state.clone();
        blocking(move || state.engine_for_node(&node_id))
            .await
            .map(Self)
    }
}

/// The engine behind the route's `{node_id}` plus a guard that interrupts the request's own reader
/// if the client goes away. The guard exists before any blocking work is queued.
pub struct Reader {
    engine: Arc<Engine>,
    guard: CancelGuard,
}

impl FromRequestParts<AppState> for Reader {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let EngineFor(engine) = EngineFor::from_request_parts(parts, state).await?;
        Ok(Self::new(engine))
    }
}

impl Reader {
    /// A reader for an engine the route found itself, as an export does from its sheets.
    pub fn new(engine: Arc<Engine>) -> Self {
        let guard = engine.guard();
        Self { engine, guard }
    }

    /// Runs `work` on a pooled reader off the async runtime. Dropping the returned future (the
    /// client left) interrupts only this request's query.
    pub async fn run<T, F>(mut self, work: F) -> ApiResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Engine, &Connection) -> ApiResult<T> + Send + 'static,
    {
        let ticket = self.guard.ticket();
        let engine = self.engine;
        let outcome = blocking(move || {
            let outcome = engine.read(Some(&ticket), |conn| work(&engine, conn));
            if ticket.is_cancelled() {
                tracing::debug!("discarded a result for a client that went away");
            }
            Ok(outcome)
        })
        .await;
        // The guard is armed whenever this future is alive, so only an abandoned request is cancelled.
        self.guard.disarm();
        outcome?.map_err(|Cancelled| ApiError::internal("Internal error"))?
    }

    /// Like [`Reader::run`] without a ticket: the work always finishes, as exports and join previews must.
    pub async fn run_to_end<T, F>(mut self, work: F) -> ApiResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Engine, &Connection) -> ApiResult<T> + Send + 'static,
    {
        let engine = self.engine;
        let outcome = blocking(move || Ok(engine.read(None, |conn| work(&engine, conn)))).await;
        self.guard.disarm();
        outcome?.map_err(|Cancelled| ApiError::internal("Internal error"))?
    }
}

/// A route capture by name; a route that lacks it is a programming error, answered 500.
async fn path_param(parts: &mut Parts, state: &AppState, name: &str) -> ApiResult<String> {
    let params = RawPathParams::from_request_parts(parts, state)
        .await
        .map_err(|_| ApiError::internal("Internal error"))?;
    params
        .iter()
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
        .ok_or_else(|| ApiError::internal("Internal error"))
}

#[cfg(test)]
mod tests;
