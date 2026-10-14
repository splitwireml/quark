//! Find: the next or previous cell of a View that contains a term, with wrap-around.

pub mod model;
mod routes;
pub mod service;

pub use routes::routes;

#[cfg(test)]
mod tests;
