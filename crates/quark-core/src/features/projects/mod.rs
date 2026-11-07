//! Projects: listing them with their source counts and creating new ones.

pub mod model;
mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
