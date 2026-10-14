//! Charts: chart requests, the per-kind chart responses and the visualize routes.

mod bar;
mod box_plot;
mod histogram;
mod metric;
pub mod model;
mod routes;
mod scatter;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
