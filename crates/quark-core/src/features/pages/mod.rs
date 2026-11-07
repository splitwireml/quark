//! Pages: one page of a dataset or of a SQL result, as JSON or Arrow.

pub mod model;
mod queries;
mod routes;
pub mod service;

pub use routes::routes;
