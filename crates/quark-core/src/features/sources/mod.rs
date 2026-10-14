//! Sources: listing them with their views and datasets, uploading or attaching files and deleting them.

pub mod attach;
pub mod model;
mod routes;
pub mod service;
pub mod uploads;

pub use routes::routes;

#[cfg(test)]
mod tests;
