pub mod gen;
pub mod mocks;

use std::env;
use std::sync::Arc;

use graphix_indexer_client::{Indexer, RealIndexer, SubgraphDeployment};
use once_cell::sync::Lazy;
use prometheus_exporter::prometheus::IntCounterVec;
use rand::rngs::{OsRng, SmallRng};
use rand::{RngCore, SeedableRng};
use url::Url;

use crate::config::IndexerConfig;

pub static TEST_SEED: Lazy<u64> = Lazy::new(|| {
    let seed = env::var("TEST_SEED")
        .map(|seed| seed.parse().expect("Invalid TEST_SEED value"))
        .unwrap_or(OsRng.next_u64());

    println!("------------------------------------------------------------------------");
    println!("TEST_SEED={}", seed);
    println!("  This value can be changed via the environment variable TEST_SEED.");
    println!("------------------------------------------------------------------------");

    seed
});

/// Test utility function to create a valid `Indexer` from an arbitrary base url.
pub fn test_indexer_from_url(url: impl Into<String>) -> Arc<impl Indexer> {
    let url: Url = url.into().parse().expect("Invalid status url");

    let mut addr = url.to_string().into_bytes();
    addr.resize(20, 0);
    let address = <[u8; 20]>::try_from(addr).unwrap().into();

    let conf = IndexerConfig {
        name: Some(url.host().unwrap().to_string()),
        address,
        index_node_endpoint: url.join("status").unwrap(),
    };
    Arc::new(RealIndexer::new(
        conf.name,
        conf.address,
        conf.index_node_endpoint.to_string(),
        IntCounterVec::new(prometheus::Opts::new("foo", "bar"), &["a", "b"]).unwrap(),
    ))
}

/// Test utility function to create a valid `SubgraphDeployment` with an arbitrary deployment
/// id/ipfs hash.
pub fn test_deployment_id(deployment: impl Into<String>) -> SubgraphDeployment {
    SubgraphDeployment(deployment.into())
}

pub fn fast_rng(seed_extra: u64) -> SmallRng {
    SmallRng::seed_from_u64(*TEST_SEED + seed_extra)
}
