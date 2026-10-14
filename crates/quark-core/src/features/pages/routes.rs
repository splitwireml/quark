//! Query routes: one page of a dataset or of a SQL result, as JSON or Arrow.

use axum::extract::Path;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};

use super::model::{Format, PageBody};
use super::service::run_page;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, Reader};
use crate::relation::request::{QueryRequest, SqlQueryRequest};
use crate::relation::target::Target;
use crate::runtime::AppState;
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
    reader: ApiResult<Reader>,
    format: Format,
    Path((_node_id, dataset)): Path<(String, String)>,
    ApiJson(request): ApiJson<QueryRequest>,
) -> ApiResult<Response> {
    page(reader, Target::Dataset(dataset), request, format).await
}

async fn sql_query(
    reader: ApiResult<Reader>,
    format: Format,
    ApiJson(request): ApiJson<SqlQueryRequest>,
) -> ApiResult<Response> {
    let target = Target::Sql(request.sql.clone());
    page(reader, target, request.query(), format).await
}

/// The reader arrives as a result so a bad body or page request is reported before an unknown node.
async fn page(
    reader: ApiResult<Reader>,
    target: Target,
    request: QueryRequest,
    format: Format,
) -> ApiResult<Response> {
    request.validate()?;
    let body = reader?
        .run(move |engine, conn| run_page(engine, conn, &target, &request, format))
        .await?;
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
