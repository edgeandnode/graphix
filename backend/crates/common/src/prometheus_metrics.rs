use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::OnceLock,
};

// It's important to use the exported crate `prometheus_exporter::prometheus`
// instead of `prometheus`, as different versions of that crate have
// incompatible global registries.
use prometheus_exporter::prometheus;

pub struct PrometheusMetrics {
    pub indexing_statuses_requests: prometheus::IntCounterVec,
    pub public_proofs_of_indexing_requests: prometheus::IntCounterVec,
}

static METRICS: OnceLock<PrometheusMetrics> = OnceLock::new();

pub fn metrics() -> &'static PrometheusMetrics {
    METRICS.get_or_init(|| {
        PrometheusMetrics::new(prometheus_exporter::prometheus::default_registry().clone())
    })
}

impl PrometheusMetrics {
    fn new(registry: prometheus::Registry) -> Self {
        let indexing_statuses_requests = prometheus::register_int_counter_vec_with_registry!(
            "indexing_statuses_requests",
            "Number of indexingStatuses requests",
            &["indexer", "success"],
            registry
        )
        .unwrap();
        let public_proofs_of_indexing_requests =
            prometheus::register_int_counter_vec_with_registry!(
                "public_proofs_of_indexing_requests",
                "Number of publicProofsOfIndexing requests",
                &["indexer", "success"],
                registry
            )
            .unwrap();

        Self {
            indexing_statuses_requests,
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
