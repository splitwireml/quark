//! The find route: the next or previous cell of a SQL result that contains a term.

use axum::routing::post;
use axum::{Json, Router};

use super::model::{CellSearch, FindResponse};
use super::service::find_cell;
use crate::error::ApiResult;
use crate::http::deps::{ApiJson, Reader};
use crate::runtime::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/nodes/{node_id}/sql/find", post(find))
}

/// The reader arrives as a result so a bad body is reported before an unknown node.
async fn find(
    reader: ApiResult<Reader>,
    ApiJson(request): ApiJson<CellSearch>,
) -> ApiResult<Json<FindResponse>> {
    request.validate()?;
    reader?
        .run(move |engine, conn| find_cell(engine, conn, &request))
        .await
        .map(Json)
}
