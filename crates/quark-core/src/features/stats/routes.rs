//! Stats routes: GET and POST for a dataset column, POST for an SQL result column.

use axum::body::Bytes;
use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

use super::service::stats;
use crate::error::{ApiError, ApiResult};
use crate::http::deps::{ApiJson, Reader};
use crate::relation::request::{QueryRequest, SqlQueryRequest};
use crate::relation::target::Target;
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats",
            get(dataset_stats_default).post(dataset_stats),
        )
        .route(
            "/api/nodes/{node_id}/sql/columns/{column}/stats",
            post(sql_stats),
        )
}

async fn dataset_stats_default(
    reader: ApiResult<Reader>,
    Path((_node_id, dataset, column)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    profile(
        reader,
        Target::Dataset(dataset),
        QueryRequest::default(),
        column,
    )
    .await
}

/// An empty body means the default request, as Python's optional body does.
async fn dataset_stats(
    reader: ApiResult<Reader>,
    Path((_node_id, dataset, column)): Path<(String, String, String)>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let request = if body.is_empty() {
        QueryRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(|error| ApiError::unprocessable(error.to_string()))?
    };
    profile(reader, Target::Dataset(dataset), request, column).await
}

async fn sql_stats(
    reader: ApiResult<Reader>,
    Path((_node_id, column)): Path<(String, String)>,
    ApiJson(request): ApiJson<SqlQueryRequest>,
) -> ApiResult<Json<Value>> {
    let target = Target::Sql(request.sql.clone());
    profile(reader, target, request.query(), column).await
}

/// The reader arrives as a result so a bad body or page request is reported before an unknown node.
async fn profile(
    reader: ApiResult<Reader>,
    target: Target,
    request: QueryRequest,
    column: String,
) -> ApiResult<Json<Value>> {
    request.validate()?;
    reader?
        .run(move |engine, conn| stats(engine, conn, &target, &request, &column))
        .await
        .map(Json)
}
