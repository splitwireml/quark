//! Column stats: a profile of one column of a dataset or SQL result, after filters and dedupe.

mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
