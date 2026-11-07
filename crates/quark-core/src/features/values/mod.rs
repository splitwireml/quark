//! Category values: the distinct values of a text column with their counts, searchable and paged.

mod queries;
mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
