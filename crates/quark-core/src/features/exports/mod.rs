//! Exports: the request that names the sheets to write, the names of the files and sheets, the CSV, XLSX, Parquet and JSON writers and the download route.

mod copy;
mod csv;
pub mod model;
pub mod names;
mod routes;
pub mod service;
mod xlsx;

pub use routes::routes;

#[cfg(test)]
mod tests;
