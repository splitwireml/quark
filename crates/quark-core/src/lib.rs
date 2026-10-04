//! Quark core: the DuckDB-backed data engine behind the desktop app.

pub mod engine;
pub mod error;
pub mod guard;
pub mod ids;
pub mod literal;
pub mod query;
pub mod registry;
pub mod sql;
pub mod values;
