//! Query routes: one page of a dataset or of a SQL result, as JSON or Arrow.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};

use super::{ApiJson, blocking};
use crate::arrow::ARROW_MEDIA_TYPE;
use crate::error::ApiResult;
use crate::page::{Format, PageBody, PageTarget, run_page};
use crate::query::{QueryRequest, SqlQueryRequest};
use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/nodes/{node_id}/datasets/{dataset}/query",
            post(dataset_query),
        )
        .route("/api/nodes/{node_id}/sql", post(sql_query))
}

async fn dataset_query(
    State(state): State<AppState>,
    Path((node_id, dataset)): Path<(String, String)>,
    headers: HeaderMap,
    ApiJson(request): ApiJson<QueryRequest>,
) -> ApiResult<Response> {
    page(
        state,
        node_id,
        PageTarget::Dataset(dataset),
        request,
        &headers,
    )
    .await
}

async fn sql_query(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    headers: HeaderMap,
    ApiJson(request): ApiJson<SqlQueryRequest>,
) -> ApiResult<Response> {
    let target = PageTarget::Sql(request.sql.clone());
    page(state, node_id, target, request.query(), &headers).await
}

fn wants_arrow(headers: &HeaderMap) -> bool {
    headers.get_all(header::ACCEPT).iter().any(|value| {
        value
            .to_str()
            .is_ok_and(|accept| accept.contains(ARROW_MEDIA_TYPE))
    })
}

async fn page(
    state: AppState,
    node_id: String,
    target: PageTarget,
    request: QueryRequest,
    headers: &HeaderMap,
) -> ApiResult<Response> {
    request.validate()?;
    let format = if wants_arrow(headers) {
        Format::Arrow
    } else {
        Format::Json
    };
    let body = blocking(move || {
        let engine = state.engine_for_node(&node_id)?;
        let mut inner = engine.lock();
        run_page(&mut inner, &target, &request, format)
    })
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
mod tests {
    use super::super::{blocking, router};
    use crate::arrow::ARROW_MEDIA_TYPE;
    use crate::engine::Dirs;
    use crate::error::ApiResult;
    use crate::ids::dataset_id;
    use crate::state::AppState;
    use arrow::ipc::reader::StreamReader;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, Request};
    use axum::routing::post;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::fs;
    use std::io::Cursor;
    use tower::ServiceExt;

    async fn panic_while_locked(State(state): State<AppState>, Path(node_id): Path<String>) {
        let _ = blocking(move || -> ApiResult<()> {
            let engine = state.engine_for_node(&node_id)?;
            let _inner = engine.lock();
            panic!("boom")
        })
        .await;
    }

    /// A started app with one CSV node `legacy` (dataset `main.data`, columns a and b).
    fn app(root: &std::path::Path) -> Router {
        let dirs = Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        };
        fs::create_dir_all(&dirs.data).unwrap();
        let file = root.join("legacy.csv");
        fs::write(&file, "a,b\n1,x\n2,y\n3,z\n").unwrap();
        let entry = json!([{
            "id": "legacy", "name": "LEGACY", "kind": "csv", "source": file.to_string_lossy(),
        }]);
        fs::write(dirs.registry_file(), entry.to_string()).unwrap();
        fs::write(dirs.projects_file(), "[]").unwrap();
        let state = AppState::load(dirs).unwrap();
        let wedge = Router::new()
            .route("/test/panic/{node_id}", post(panic_while_locked))
            .with_state(state.clone());
        router(state).merge(wedge)
    }

    async fn post_json(
        app: &Router,
        uri: &str,
        accept: Option<&str>,
        body: Value,
    ) -> (u16, HeaderMap, Vec<u8>) {
        let mut request = Request::post(uri).header("content-type", "application/json");
        if let Some(accept) = accept {
            request = request.header("accept", accept);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes().to_vec();
        (parts.status.as_u16(), parts.headers, bytes)
    }

    fn query_uri() -> String {
        format!(
            "/api/nodes/legacy/datasets/{}/query",
            dataset_id("main", "data")
        )
    }

    fn as_json(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[tokio::test]
    async fn query_route_json_and_arrow() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path());
        let request = json!({"page_size": 2, "sorts": [{"column": "a", "direction": "desc"}]});

        let (status, headers, bytes) = post_json(&app, &query_uri(), None, request.clone()).await;
        assert_eq!(status, 200);
        assert_eq!(headers["content-type"], "application/json");
        let page = as_json(&bytes);
        assert_eq!(
            page["rows"],
            json!([{"a": 3, "b": "z"}, {"a": 2, "b": "y"}])
        );
        assert_eq!(
            (page["total_rows"].as_u64(), page["total_pages"].as_u64()),
            (Some(3), Some(2))
        );

        let accept = format!("text/html, {ARROW_MEDIA_TYPE};q=0.9");
        let (status, headers, bytes) = post_json(&app, &query_uri(), Some(&accept), request).await;
        assert_eq!(status, 200);
        assert_eq!(headers["content-type"], ARROW_MEDIA_TYPE);
        assert_eq!(headers["vary"], "Accept");
        let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        assert!(reader.schema().metadata().contains_key("quark"));
        let rows: usize = reader.map(|batch| batch.unwrap().num_rows()).sum();
        assert_eq!(rows, 2);

        let (status, _, bytes) = post_json(
            &app,
            "/api/nodes/legacy/datasets/bm9wZQ/query",
            None,
            json!({}),
        )
        .await;
        assert_eq!(status, 404);
        assert_eq!(as_json(&bytes), json!({"detail": "Dataset not found"}));
        let (status, _, bytes) = post_json(
            &app,
            "/api/nodes/nope/sql",
            None,
            json!({"sql": "SELECT 1"}),
        )
        .await;
        assert_eq!(status, 404);
        assert_eq!(as_json(&bytes), json!({"detail": "Node not found"}));
    }

    #[tokio::test]
    async fn sql_route_rejects_non_select() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path());
        let uri = "/api/nodes/legacy/sql";

        let (status, _, bytes) = post_json(
            &app,
            uri,
            None,
            json!({"sql": "SELECT a FROM data WHERE a > 1"}),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(as_json(&bytes)["total_rows"], 2);

        for sql in [
            "DROP VIEW data",
            "SELECT 1; SELECT 2",
            "COPY data TO 'x.csv'",
        ] {
            let (status, _, bytes) = post_json(&app, uri, None, json!({"sql": sql})).await;
            assert_eq!(status, 422, "{sql}");
            assert_eq!(
                as_json(&bytes),
                json!({"detail": "SQL accepts only one read-only SELECT query"}),
                "{sql}"
            );
        }
        let (status, _, _) = post_json(&app, uri, None, json!({"query": "SELECT 1"})).await;
        assert_eq!(status, 422);
    }

    #[tokio::test]
    async fn invalid_paging_is_422() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path());

        for paging in [
            json!({"page": 0}),
            json!({"page": -1}),
            json!({"page_size": 0}),
            json!({"page_size": 1001}),
        ] {
            let mut sql = paging.clone();
            sql["sql"] = json!("SELECT 1");
            for (uri, body) in [
                (query_uri(), paging.clone()),
                ("/api/nodes/legacy/sql".to_owned(), sql),
            ] {
                let (status, _, bytes) = post_json(&app, &uri, None, body).await;
                assert_eq!(status, 422, "{uri} {paging}");
                assert_eq!(
                    as_json(&bytes),
                    json!({"detail": "Invalid page or page size"})
                );
            }
        }
    }

    #[tokio::test]
    async fn panic_does_not_wedge_the_engine() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path());

        let (status, _, _) = post_json(&app, "/test/panic/legacy", None, json!({})).await;
        assert_eq!(status, 200);
        let (status, _, bytes) = post_json(&app, &query_uri(), None, json!({})).await;
        assert_eq!(status, 200);
        assert_eq!(as_json(&bytes)["total_rows"], 3);
    }
}
