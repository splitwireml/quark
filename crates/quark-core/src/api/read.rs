//! Read routes: projects, sources, views, nodes and datasets.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::{ApiJson, blocking};
use crate::error::ApiResult;
use crate::mount::ViewInfo;
use crate::state::{
    AppState, DatasetEntry, LegacyNode, ProjectSummary, SourceDetail, SourceSummary,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/{project_id}/sources", get(list_sources))
        .route(
            "/api/projects/{project_id}/sources/{source_id}",
            get(get_source),
        )
        .route("/api/projects/{project_id}/views", get(list_views))
        .route("/api/nodes", get(list_nodes))
        .route("/api/nodes/{node_id}/datasets", get(list_datasets))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectCreate {
    name: String,
}

async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<ProjectSummary>>> {
    blocking(move || Ok(state.projects())).await.map(Json)
}

async fn create_project(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ProjectCreate>,
) -> ApiResult<(StatusCode, Json<ProjectSummary>)> {
    let project = blocking(move || state.create_project(&request.name)).await?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn list_sources(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<SourceSummary>>> {
    blocking(move || state.project_sources(&project_id))
        .await
        .map(Json)
}

async fn get_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> ApiResult<Json<SourceDetail>> {
    blocking(move || state.source_detail(&project_id, &source_id))
        .await
        .map(Json)
}

async fn list_views(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<ViewInfo>>> {
    blocking(move || state.project_views(&project_id))
        .await
        .map(Json)
}

async fn list_nodes(State(state): State<AppState>) -> ApiResult<Json<Vec<LegacyNode>>> {
    blocking(move || Ok(state.legacy_nodes())).await.map(Json)
}

async fn list_datasets(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> ApiResult<Json<Vec<DatasetEntry>>> {
    blocking(move || state.datasets(&node_id)).await.map(Json)
}

#[cfg(test)]
mod tests {
    use super::super::router;
    use super::*;
    use crate::engine::Dirs;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::fs;
    use tower::ServiceExt;

    /// A started app over a temp data folder holding the given registry and projects.
    fn app(root: &std::path::Path, registry: &[Value], projects: &[Value]) -> Router {
        let dirs = Dirs {
            data: root.join("data"),
            cache: root.join("cache"),
        };
        fs::create_dir_all(&dirs.data).unwrap();
        fs::write(dirs.registry_file(), json!(registry).to_string()).unwrap();
        fs::write(dirs.projects_file(), json!(projects).to_string()).unwrap();
        router(AppState::load(dirs).unwrap())
    }

    fn entry(root: &std::path::Path, id: &str, project: Option<&str>) -> Value {
        let file = root.join(format!("{id}.csv"));
        fs::write(&file, "a,b\n1,x\n2,y\n").unwrap();
        let mut value = json!({
            "id": id, "name": id.to_uppercase(), "kind": "csv", "source": file.to_string_lossy(),
        });
        if let Some(project) = project {
            value["project_id"] = json!(project);
        }
        value
    }

    async fn send(app: &Router, method: Method, uri: &str, body: Option<&str>) -> (u16, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        let body = match body {
            Some(text) => {
                request = request.header("content-type", "application/json");
                Body::from(text.to_owned())
            }
            None => Body::empty(),
        };
        let response = app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn get_json(app: &Router, uri: &str) -> (u16, Value) {
        send(app, Method::GET, uri, None).await
    }

    #[tokio::test]
    async fn projects_routes() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path(), &[], &[]);

        let (status, listed) = get_json(&app, "/api/projects").await;
        assert_eq!(status, 200);
        assert_eq!(
            listed,
            json!([{"id": "default", "name": "Default", "node_id": "project_default", "source_count": 0}])
        );

        let (status, created) = send(
            &app,
            Method::POST,
            "/api/projects",
            Some(r#"{"name":"  Quarterly  "}"#),
        )
        .await;
        assert_eq!(status, 201);
        assert_eq!(created["name"], "Quarterly");
        assert_eq!(created["source_count"], 0);
        assert_eq!(
            created["node_id"],
            format!("project_{}", created["id"].as_str().unwrap())
        );
        let (_, listed) = get_json(&app, "/api/projects").await;
        assert_eq!(listed.as_array().unwrap().len(), 2);
        assert_eq!(listed[1], created);

        let (status, blank) = send(
            &app,
            Method::POST,
            "/api/projects",
            Some(r#"{"name":"  "}"#),
        )
        .await;
        assert_eq!(
            (status, blank),
            (422, json!({"detail": "Project name is required"}))
        );
        for bad in [
            r#"{}"#,
            r#"{"name":"x","extra":1}"#,
            r#"{"name":"#,
            r#"{"name":3}"#,
        ] {
            let (status, body) = send(&app, Method::POST, "/api/projects", Some(bad)).await;
            assert_eq!(status, 422, "{bad}");
            assert!(body["detail"].is_string(), "{bad}");
        }
        let request = Request::post("/api/projects")
            .body(Body::from("{}"))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn source_and_view_routes() {
        let root = tempfile::tempdir().unwrap();
        let project = json!({"id": "p", "name": "P", "node_id": "project_p"});
        let app = app(
            root.path(),
            &[
                entry(root.path(), "one", Some("p")),
                entry(root.path(), "two", Some("p")),
            ],
            &[project],
        );

        let (status, sources) = get_json(&app, "/api/projects/p/sources").await;
        assert_eq!(status, 200);
        assert_eq!(
            sources,
            json!([{"id": "one", "name": "ONE"}, {"id": "two", "name": "TWO"}])
        );

        let (status, detail) = get_json(&app, "/api/projects/p/sources/two").await;
        assert_eq!(status, 200);
        assert_eq!(
            (
                detail["id"].as_str(),
                detail["kind"].as_str(),
                detail["project_id"].as_str()
            ),
            (Some("two"), Some("csv"), Some("p"))
        );
        assert_eq!(detail["views"].as_array().unwrap().len(), 1);
        assert_eq!(detail["views"][0]["source_id"], "two");

        let (status, views) = get_json(&app, "/api/projects/p/views").await;
        assert_eq!(status, 200);
        let owners: Vec<_> = views
            .as_array()
            .unwrap()
            .iter()
            .map(|view| view["source_id"].as_str().unwrap())
            .collect();
        assert_eq!(owners, ["one", "two"]);

        for (uri, detail) in [
            ("/api/projects/nope/sources", "Project not found"),
            ("/api/projects/nope/views", "Project not found"),
            ("/api/projects/p/sources/nope", "Source not found"),
        ] {
            let (status, body) = get_json(&app, uri).await;
            assert_eq!((status, body), (404, json!({"detail": detail})), "{uri}");
        }
    }

    #[tokio::test]
    async fn unknown_api_route_is_501() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path(), &[], &[]);

        let (status, body) = get_json(&app, "/api/export").await;
        assert_eq!(
            (status, body),
            (501, json!({"detail": "Not in the desktop build yet"}))
        );
        let (status, _) = send(&app, Method::GET, "/api/nodes/upload", None).await;
        assert_eq!(status, 501);
        let (status, body) = send(
            &app,
            Method::POST,
            "/api/projects/default/sources/attach",
            Some("{}"),
        )
        .await;
        assert_eq!(status, 501);
        assert_eq!(body, json!({"detail": "Not in the desktop build yet"}));
        let (status, _) = get_json(&app, "/elsewhere").await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn datasets_route_lists_columns() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path(), &[entry(root.path(), "legacy", None)], &[]);

        let (status, nodes) = get_json(&app, "/api/nodes").await;
        assert_eq!(status, 200);
        assert_eq!(nodes.as_array().unwrap().len(), 1);
        assert_eq!(
            (
                nodes[0]["id"].as_str(),
                nodes[0]["name"].as_str(),
                nodes[0]["kind"].as_str()
            ),
            (Some("legacy"), Some("LEGACY"), Some("csv"))
        );

        let (status, datasets) = get_json(&app, "/api/nodes/legacy/datasets").await;
        assert_eq!(status, 200);
        assert_eq!(
            datasets,
            json!([{
                "id": "WyJtYWluIiwiZGF0YSJd", "name": "data", "schema": "main",
                "type": "VIEW", "columns": ["a", "b"],
            }])
        );

        let (status, body) = get_json(&app, "/api/nodes/nope/datasets").await;
        assert_eq!((status, body), (404, json!({"detail": "Node not found"})));
    }
}
