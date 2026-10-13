//! Quark core: the DuckDB-backed data engine behind the desktop app.

pub mod api;
pub mod cache;
pub mod cancel;
pub mod engine;
pub mod error;
pub mod mount;
pub mod page;
pub mod registry;
pub mod relation;
pub mod secure;
pub mod shared;
pub mod state;
