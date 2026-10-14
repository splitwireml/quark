//! Category values routes: the values of a text column of a dataset or of a SQL result.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use super::service::{ValuesPage, category_values};
use crate::error::{ApiError, ApiResult};
use crate::http::deps::{ApiJson, Reader};
use crate::relation::target::Target;
use crate::runtime::AppState;

/// The `{sql}` body of the SQL route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SqlBody {
    sql: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values",
            get(dataset_values),
        )
        .route(
            "/api/nodes/{node_id}/sql/columns/{column}/values",
            post(sql_values),
        )
}

async fn dataset_values(
    reader: ApiResult<Reader>,
    page: Result<Query<ValuesPage>, QueryRejection>,
    Path((_node_id, dataset, column)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    values(reader, page, Target::Dataset(dataset), column).await
}

async fn sql_values(
    reader: ApiResult<Reader>,
    page: Result<Query<ValuesPage>, QueryRejection>,
    Path((_node_id, column)): Path<(String, String)>,
    ApiJson(request): ApiJson<SqlBody>,
) -> ApiResult<Json<Value>> {
    values(reader, page, Target::Sql(request.sql), column).await
}

/// The reader arrives as a result so a bad query string is reported before an unknown node.
async fn values(
    reader: ApiResult<Reader>,
    page: Result<Query<ValuesPage>, QueryRejection>,
    target: Target,
    column: String,
) -> ApiResult<Json<Value>> {
    let Query(page) = page.map_err(|error| ApiError::unprocessable(error.body_text()))?;
    page.validate()?;
    let body = reader?
        .run(move |engine, conn| category_values(engine, conn, &target, &column, &page))
        .await?;
    Ok(Json(body))
}
