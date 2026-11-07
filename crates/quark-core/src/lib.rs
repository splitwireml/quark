//! Quark core: the DuckDB-backed data engine behind the desktop app.

pub mod error;
pub mod features;
pub mod http;
pub mod relation;
pub mod runtime;
pub mod shared;

pub use http::router;
