//! Query routes: one page of a dataset or of a SQL result, as JSON or Arrow.

use axum::extract::Path;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};

use super::model::{Format, PageBody};
use super::service::run_page;
use crate::error::{ApiError, ApiResult};
use crate::http::deps::{ApiJson, EngineFor, blocking};
use crate::relation::request::{QueryRequest, SqlQueryRequest};
use crate::relation::target::Target;
use crate::runtime::AppState;
use crate::runtime::cancel::Cancelled;
use crate::shared::encode::arrow::ARROW_MEDIA_TYPE;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/nodes/{node_id}/datasets/{dataset}/query",
            post(dataset_query),
        )
        .route("/api/nodes/{node_id}/sql", post(sql_query))
}

async fn dataset_query(
    engine: ApiResult<EngineFor>,
    format: Format,
    Path((_node_id, dataset)): Path<(String, String)>,
    ApiJson(request): ApiJson<QueryRequest>,
) -> ApiResult<Response> {
    page(engine, Target::Dataset(dataset), request, format).await
}

async fn sql_query(
    engine: ApiResult<EngineFor>,
    format: Format,
    ApiJson(request): ApiJson<SqlQueryRequest>,
) -> ApiResult<Response> {
    let target = Target::Sql(request.sql.clone());
    page(engine, target, request.query(), format).await
}

/// The engine arrives as a result so a bad body or page request is reported before an unknown node.
async fn page(
    engine: ApiResult<EngineFor>,
    target: Target,
    request: QueryRequest,
    format: Format,
) -> ApiResult<Response> {
    request.validate()?;
    let EngineFor(engine) = engine?;
    // Dropping this future (the client left) drops the armed guard, which interrupts the query.
    let mut guard = engine.guard();
    let ticket = guard.ticket();
    let outcome = blocking(move || {
        let outcome = engine.read(Some(&ticket), |conn| {
            run_page(&engine, conn, &target, &request, format)
        });
        if ticket.is_cancelled() {
            tracing::debug!("discarded a page for a client that went away");
        }
        Ok(outcome)
    })
    .await;
    guard.disarm();
    // The guard is armed whenever this future is alive, so only an abandoned request is cancelled.
    let body = outcome?.map_err(|Cancelled| ApiError::internal("Internal error"))??;
    Ok(match body {
        PageBody::Json(value) => Json(value).into_response(),
        PageBody::Arrow(bytes) => (
            [
                (header::CONTENT_TYPE, ARROW_MEDIA_TYPE),
                (header::VARY, "Accept"),
            ],
            bytes,
        )
            .into_response(),
    })
}

#[cfg(test)]
mod tests;
