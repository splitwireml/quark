//! Sources: listing them with their views and datasets, uploading files and deleting them.

pub mod model;
mod routes;
pub mod service;
pub mod uploads;

pub use routes::routes;

#[cfg(test)]
mod tests;
