//! Previews a join: checks the request, resolves both sides in one engine and counts rows and key uniqueness.

use std::sync::Arc;

use duckdb::Connection;

use super::cross;
use super::model::{Identity, JoinPreview, JoinReference, JoinWorkspaceRequest, Relationship};
use super::queries::{has_unique_keys, inner_join_rows, row_count};
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::{datasets, describe};
use crate::relation::target::{Target, resolve};
use crate::runtime::AppState;
use crate::runtime::cache::stats::stats_key;
use crate::runtime::cancel::Cancelled;
use crate::runtime::engine::{Engine, EngineKind};
use crate::runtime::registry::DEFAULT_PROJECT_ID;
use crate::runtime::state::invalid_value;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::{quote_ident, sql_string};

/// One resolved side of the join: the table expression to read, its columns and how to name it.
struct Side {
    table: String,
    columns: Vec<ColumnMeta>,
    identity: Identity,
}

/// The request checks that need no engine, in Python's order.
pub fn validate(request: &JoinWorkspaceRequest) -> ApiResult<()> {
    if request.left_keys.len() != request.right_keys.len() {
        return Err(ApiError::unprocessable("Join key counts must match"));
    }
    if has_duplicates(&request.left_keys) || has_duplicates(&request.right_keys) {
        return Err(ApiError::unprocessable("Join keys must be unique"));
    }
    for reference in [&request.left, &request.right] {
        if reference.dataset.is_some() == reference.sql.is_some() {
            return Err(ApiError::unprocessable(
                "Join reference requires exactly one of dataset or sql",
            ));
        }
    }
    Ok(())
}

fn has_duplicates(keys: &[String]) -> bool {
    keys.iter()
        .enumerate()
        .any(|(index, key)| keys[..index].contains(key))
}

/// Previews the join in the one engine both sides live in, on a pooled reader that always finishes.
pub fn preview(state: &AppState, request: &JoinWorkspaceRequest) -> ApiResult<JoinPreview> {
    if let Some(cross) = cross::open(state, request)? {
        let result = run_on(&cross.engine, &cross.request)?;
        state.keep_join(&cross.id, &cross.engine);
        return Ok(result);
    }
    let engine = shared_engine(state, request)?;
    run_on(&engine, request)
}

fn run_on(engine: &Engine, request: &JoinWorkspaceRequest) -> ApiResult<JoinPreview> {
    engine
        .read(None, |conn| preview_on(engine, conn, request))
        .map_err(|Cancelled| ApiError::internal("Internal error"))?
}

/// The engine both sides run in. SQL sides may name a project workspace; dataset sides only default-project sources.
/// Datasets of two different sources never get here: they run in a join engine.
fn shared_engine(state: &AppState, request: &JoinWorkspaceRequest) -> ApiResult<Arc<Engine>> {
    let (left, right) = (&request.left, &request.right);
    if left.sql.is_some() || right.sql.is_some() {
        let left_engine = state.engine_for_node(&left.node_id)?;
        let right_engine = state.engine_for_node(&right.node_id)?;
        if !Arc::ptr_eq(&left_engine, &right_engine) {
            return Err(ApiError::unprocessable(
                "SQL joins require one project workspace",
            ));
        }
        return Ok(left_engine);
    }
    let is_active = |reference: &JoinReference| {
        state
            .catalog()
            .sources_of(DEFAULT_PROJECT_ID)
            .any(|source| source.id == reference.node_id)
    };
    if !is_active(left) || !is_active(right) {
        return Err(ApiError::not_found("Node not found"));
    }
    state.engine_for_node(&left.node_id)
}

/// Resolves both sides on `conn` and counts what the join would produce.
pub(super) fn preview_on(
    engine: &Engine,
    conn: &Connection,
    request: &JoinWorkspaceRequest,
) -> ApiResult<JoinPreview> {
    let left = resolve_side(conn, &request.left)?;
    let right = resolve_side(conn, &request.right)?;
    if !has_columns(&left.columns, &request.left_keys)
        || !has_columns(&right.columns, &request.right_keys)
    {
        return Err(ApiError::unprocessable("Join key not found"));
    }
    let left_rows = rows_of(engine, conn, &left.table)?;
    let right_rows = rows_of(engine, conn, &right.table)?;
    let (relationship, output_rows) = if request.left_keys.is_empty() {
        (
            Relationship::Cartesian,
            left_rows.saturating_mul(right_rows),
        )
    } else {
        let left_unique =
            has_unique_keys(conn, &left.table, &request.left_keys).map_err(invalid_value)?;
        let right_unique =
            has_unique_keys(conn, &right.table, &request.right_keys).map_err(invalid_value)?;
        let output = inner_join_rows(
            conn,
            &left.table,
            &right.table,
            &request.left_keys,
            &request.right_keys,
        )
        .map_err(invalid_value)?;
        (classify(left_unique, right_unique), output)
    };
    Ok(JoinPreview {
        node_id: node_id(engine).to_owned(),
        left: left.identity,
        right: right.identity,
        left_rows,
        right_rows,
        output_rows,
        relationship,
        cartesian_risk: matches!(
            relationship,
            Relationship::Cartesian | Relationship::ManyToMany
        ),
    })
}

fn classify(left_unique: bool, right_unique: bool) -> Relationship {
    match (left_unique, right_unique) {
        (true, true) => Relationship::OneToOne,
        (true, false) => Relationship::OneToMany,
        (false, true) => Relationship::ManyToOne,
        (false, false) => Relationship::ManyToMany,
    }
}

fn has_columns(columns: &[ColumnMeta], keys: &[String]) -> bool {
    keys.iter()
        .all(|key| columns.iter().any(|column| column.name == *key))
}

fn node_id(engine: &Engine) -> &str {
    match &engine.kind {
        EngineKind::Workspace { node_id, .. } | EngineKind::Node { node_id } => node_id,
    }
}

/// A dataset reads as its table; a SQL text is guarded and read through `query()` with the text inline.
fn resolve_side(conn: &Connection, reference: &JoinReference) -> ApiResult<Side> {
    if let Some(sql) = &reference.sql {
        let resolved = resolve(conn, &Target::Sql(sql.clone()))?;
        let sql = resolved.plain_sql.unwrap_or_else(|| sql.clone());
        return Ok(Side {
            table: format!("query({})", sql_string(&sql)),
            columns: resolved.columns,
            identity: Identity::View {
                name: reference
                    .name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "View".to_owned()),
                sql,
            },
        });
    }
    let id = reference.dataset.as_deref().unwrap_or_default();
    let item = datasets(conn)
        .map_err(invalid_value)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| ApiError::not_found("Dataset not found"))?;
    let table = format!("{}.{}", quote_ident(&item.schema), quote_ident(&item.name));
    let columns = describe(conn, &table, &[]).map_err(invalid_value)?;
    Ok(Side {
        table,
        columns,
        identity: Identity::Dataset {
            schema: item.schema,
            name: item.name,
        },
    })
}

/// The row count a page of this table already cached, or a fresh `count(*)`.
fn rows_of(engine: &Engine, conn: &Connection, table: &str) -> ApiResult<u64> {
    let key = stats_key(&format!("(SELECT * FROM {table})"), &[]);
    let cached = engine.stats().get(&key);
    match cached {
        Some(entry) => Ok(entry.total_rows),
        None => row_count(conn, table).map_err(invalid_value),
    }
}
