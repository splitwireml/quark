//! Caches that outlive a single query: the on-disk columnar files and per-engine page stats.

pub mod columnar;
pub mod results;
pub mod stats;
