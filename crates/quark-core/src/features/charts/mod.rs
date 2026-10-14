//! Charts: chart requests, the per-kind chart responses and the visualize routes.

mod box_plot;
mod metric;
pub mod model;
mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
