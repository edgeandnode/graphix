pub mod block_choice;
pub mod config;
pub mod graphql_api;
pub mod indexing_loop;
mod prometheus_metrics;

#[cfg(feature = "tests")]
pub mod test_utils;

pub use prometheus_metrics::{metrics, PrometheusExporter, PrometheusMetrics};

pub const GRAPHIX_VERSION: &str = env!("CARGO_PKG_VERSION");
