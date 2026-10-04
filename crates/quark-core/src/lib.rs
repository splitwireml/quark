//! Quark core: the DuckDB-backed data engine behind the desktop app.

pub mod api;
pub mod arrow;
pub mod cache;
pub mod cancel;
pub mod engine;
pub mod error;
pub mod guard;
pub mod ids;
pub mod literal;
pub mod mount;
pub mod naming;
pub mod page;
pub mod query;
pub mod registry;
pub mod secure;
pub mod sql;
pub mod state;
pub mod values;
