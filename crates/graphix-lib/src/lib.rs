pub mod block_choice;
pub mod config;
pub mod graphql_api;
mod prometheus_metrics;
pub mod queries;
mod unforgiving_indexer_client;

#[cfg(feature = "tests")]
pub mod test_utils;

pub use prometheus_metrics::{metrics, PrometheusExporter, PrometheusMetrics};

pub const GRAPHIX_VERSION: &str = env!("CARGO_PKG_VERSION");
