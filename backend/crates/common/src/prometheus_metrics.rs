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

#[derive(Debug)]
pub struct PrometheusExporter {
    _exporter: prometheus_exporter::Exporter,
}

impl PrometheusExporter {
    pub fn start(port: u16, registry: prometheus::Registry) -> anyhow::Result<Self> {
        let binding = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
        let exporter = {
            let mut builder = prometheus_exporter::Builder::new(binding);
            builder.with_registry(registry);
            builder.start()?
        };

        Ok(Self {
            _exporter: exporter,
        })
    }
}
