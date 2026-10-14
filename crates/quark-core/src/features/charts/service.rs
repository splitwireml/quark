//! Chart dispatch: filters the target once, runs the requested kind and caches the whole response.

use std::time::Instant;

use duckdb::Connection;
use serde_json::Value;

use super::model::{ChartKind, ChartSpec, VisualizeRequest};
use super::{bar, box_plot, histogram, metric};
use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::{Relation, filtered};
use crate::relation::target::Target;
use crate::runtime::engine::Engine;

/// Builds the chart `request.spec` asks for over `target` with the request's filters and dedupe applied.
/// Every response carries `elapsed_ms`; a cached one reports the time of its own lookup.
pub fn visualize(
    engine: &Engine,
    conn: &Connection,
    target: &Target,
    request: &VisualizeRequest,
) -> ApiResult<Value> {
    let started = Instant::now();
    let relation = filtered(conn, target, &request.query())?;
    let spec = &request.spec;
    let compute = || {
        let mut response = chart(engine, conn, &relation, spec)?;
        if let Value::Object(body) = &mut response {
            let millis = (started.elapsed().as_secs_f64() * 1_000_000.0).round() / 1000.0;
            body.insert("elapsed_ms".to_owned(), Value::from(millis));
        }
        Ok(response)
    };
    if !formula_is_cacheable(engine, conn, &relation, spec) {
        return compute();
    }
    // ChartSpec is deserialize-only, so its Debug text stands in for the spec in the cache key.
    let detail = Value::String(format!("{spec:?}"));
    engine.cached_response(conn, "chart", &relation, &detail, compute)
}

/// A metric formula is SQL the cache check has not seen: it is held to the same skip rules as the
/// relation (volatile functions, `current_*`, table functions, nested `query()` and file scans),
/// as the response is built from both.
fn formula_is_cacheable(
    engine: &Engine,
    conn: &Connection,
    relation: &Relation,
    spec: &ChartSpec,
) -> bool {
    let expression = spec.expression.as_deref().map_or("", str::trim);
    if expression.is_empty() {
        return true;
    }
    // The formula travels with the params so the cache check's nested query() and file-path
    // rules, which read the params only, see it too.
    let params_json = format!(
        "{} {}",
        Value::Array(relation.params.clone()),
        Value::String(expression.to_owned())
    );
    engine
        .is_cacheable(conn, &relation.sql, &params_json)
        .unwrap_or(false)
}

fn chart(
    engine: &Engine,
    conn: &Connection,
    relation: &Relation,
    spec: &ChartSpec,
) -> ApiResult<Value> {
    match spec.chart {
        ChartKind::Metric => metric::chart(conn, relation, spec),
        ChartKind::Bar | ChartKind::Pie => bar::chart(conn, relation, spec),
        ChartKind::Histogram => histogram::chart(engine, conn, relation, spec),
        ChartKind::Box => box_plot::chart(conn, relation, spec),
        ChartKind::Scatter => unavailable("scatter"),
        ChartKind::Line => unavailable("line"),
    }
}

fn unavailable(kind: &str) -> ApiResult<Value> {
    Err(ApiError::unprocessable(format!(
        "Chart type {kind} is not available yet"
    )))
}
