//! Joins: a preview of how two datasets or SQL views would join, with the row counts and relationship.

mod cross;
pub mod model;
mod queries;
mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
