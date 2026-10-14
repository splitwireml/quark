//! Chart routes: a chart over a dataset or over a SQL result.

use axum::extract::Path;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;

use super::model::{SqlVisualizeRequest, VisualizeRequest};
use super::service::visualize;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, Reader};
use crate::relation::target::Target;
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/nodes/{node_id}/datasets/{dataset}/visualize",
            post(dataset_chart),
        )
        .route("/api/nodes/{node_id}/sql/visualize", post(sql_chart))
}

async fn dataset_chart(
    reader: ApiResult<Reader>,
    Path((_node_id, dataset)): Path<(String, String)>,
    ApiJson(request): ApiJson<VisualizeRequest>,
) -> ApiResult<Json<Value>> {
    chart(reader, Target::Dataset(dataset), request).await
}

async fn sql_chart(
    reader: ApiResult<Reader>,
    ApiJson(request): ApiJson<SqlVisualizeRequest>,
) -> ApiResult<Json<Value>> {
    request.validate()?;
    let SqlVisualizeRequest {
        sql,
        spec,
        page,
        page_size,
        filters,
        sorts,
        dedupe_columns,
    } = request;
    let request = VisualizeRequest {
        spec,
        page,
        page_size,
        filters,
        sorts,
        dedupe_columns,
    };
    chart(reader, Target::Sql(sql), request).await
}

/// The reader arrives as a result so a bad body or page request is reported before an unknown node.
async fn chart(
    reader: ApiResult<Reader>,
    target: Target,
    request: VisualizeRequest,
) -> ApiResult<Json<Value>> {
    request.validate()?;
    let response = reader?
        .run(move |engine, conn| visualize(engine, conn, &target, &request))
        .await?;
    Ok(Json(response))
}
