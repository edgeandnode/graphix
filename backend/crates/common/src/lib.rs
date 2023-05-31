pub mod api_types;
pub mod bisect;
mod config;
pub mod db;
mod indexer;
pub mod indexing_statuses;
pub mod modes;
pub mod proofs_of_indexing;
mod types;

#[cfg(any(test, feature = "tests"))]
pub mod tests;

pub mod prelude {
    pub use super::config::*;
    pub use super::db;
    pub use super::indexer::*;
    pub use super::modes;
    pub use super::types::*;
}

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

// It's important to use the exported crate `prometheus_exporter::prometheus`
// instead of `prometheus`, as different versions of that crate have
// incompatible global registries.
use prometheus_exporter::prometheus;

pub struct PrometheusMetrics {
    pub public_proofs_of_indexing_requests: prometheus::IntCounterVec,
}

impl PrometheusMetrics {
    pub fn new(registry: prometheus::Registry) -> Self {
        let public_proofs_of_indexing_requests =
            prometheus::register_int_counter_vec_with_registry!(
                "public_proofs_of_indexing_requests",
                "Number of public_proofs_of_indexing requests",
                &["indexer", "statutes", "success"],
                registry
            )
            .unwrap();

        Self {
            public_proofs_of_indexing_requests,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrometheusExporter {
    _port: u16,

    pub public_proofs_of_indexing_requests_counter: prometheus::IntCounter,
}

impl PrometheusExporter {
    pub fn start(&self, port: u16) -> Self {
        let binding = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
        prometheus_exporter::start(binding).unwrap();

        let public_proofs_of_indexing_requests_counter = prometheus::register_int_counter!(
            "public_proofs_of_indexing_requests_counter",
            "Number of public_proofs_of_indexing requests"
        )
        .unwrap();

        Self {
            _port: port,
            public_proofs_of_indexing_requests_counter,
        }
    }
}
