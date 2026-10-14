//! Opens the join engine behind a preview whose two datasets live in different sources.

use std::sync::Arc;

use super::model::{JoinReference, JoinWorkspaceRequest};
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::{DatasetInfo, datasets};
use crate::runtime::AppState;
use crate::runtime::cancel::Cancelled;
use crate::runtime::engine::Engine;
use crate::runtime::registry::{DEFAULT_PROJECT_ID, SourceRecord};
use crate::runtime::state::invalid_value;
use crate::shared::ids::dataset_id;

/// A fresh join engine and the request rewritten to name the datasets inside it.
pub(super) struct Cross {
    pub id: String,
    pub engine: Arc<Engine>,
    pub request: JoinWorkspaceRequest,
}

/// The join engine for a request with datasets from two different default-project sources, if it is one.
pub(super) fn open(state: &AppState, request: &JoinWorkspaceRequest) -> ApiResult<Option<Cross>> {
    let (left, right) = (&request.left, &request.right);
    if left.sql.is_some() || right.sql.is_some() || left.node_id == right.node_id {
        return Ok(None);
    }
    let left_source = active_source(state, &left.node_id)?;
    let right_source = active_source(state, &right.node_id)?;
    let left_item = dataset_of(state, &left_source, left)?;
    let right_item = dataset_of(state, &right_source, right)?;
    let (id, engine) = state.open_join((&left_source, &left_item), (&right_source, &right_item))?;
    let rewritten = JoinWorkspaceRequest {
        left: inside("left_source", left, &left_item),
        right: inside("right_source", right, &right_item),
        left_keys: request.left_keys.clone(),
        right_keys: request.right_keys.clone(),
    };
    Ok(Some(Cross {
        id,
        engine,
        request: rewritten,
    }))
}

fn active_source(state: &AppState, node_id: &str) -> ApiResult<SourceRecord> {
    state
        .catalog()
        .sources_of(DEFAULT_PROJECT_ID)
        .find(|source| source.id == node_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("Node not found"))
}

/// The dataset `reference` names, as its own source lists it.
fn dataset_of(
    state: &AppState,
    source: &SourceRecord,
    reference: &JoinReference,
) -> ApiResult<DatasetInfo> {
    let engine = state.engine_for_node(&source.id)?;
    let listed = engine
        .read(None, datasets)
        .map_err(|Cancelled| ApiError::internal("Internal error"))?
        .map_err(invalid_value)?;
    listed
        .into_iter()
        .find(|item| Some(&item.id) == reference.dataset.as_ref())
        .ok_or_else(|| ApiError::not_found("Dataset not found"))
}

/// `reference` pointing at the same dataset under `schema` in the join engine.
fn inside(schema: &str, reference: &JoinReference, item: &DatasetInfo) -> JoinReference {
    JoinReference {
        dataset: Some(dataset_id(schema, &item.name)),
        ..reference.clone()
    }
}
