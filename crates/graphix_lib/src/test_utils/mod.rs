pub mod gen;
pub mod mocks;

use std::env;
use std::str::FromStr;
use std::sync::Arc;

use graphix_common_types::IpfsCid;
use graphix_indexer_client::{IndexerClient, RealIndexer};
use once_cell::sync::Lazy;
use prometheus_exporter::prometheus::IntCounterVec;
use rand::rngs::{OsRng, SmallRng};
use rand::{RngCore, SeedableRng};
use url::Url;

use crate::config::IndexerConfig;

pub mod deployments {
    pub const ARB1_PREMIA_BLUE: &str = "QmdHQVHirs3yPygcgo3HNttXaFCS4pnoGiMx3aKXr192En";
    pub const ARB1_QUICKSWAP_V3: &str = "QmQEYSGSD8t7jTw4gS2dwC4DLvyZceR9fYQ432Ff1hZpCp";
    pub const ARB1_LIDO: &str = "Qmd3vU6y6pxxXPrvVWRZMN9soNB8AFQCEnqPa9jMSZZDEG";

    /// Extremely low curation signal, basically no indexer has it.
    pub const FUSE_TO_ETHEREUM_AMB: &str = "QmYU3Exnta8H52vWUFhGQi6Qm8LhXr5LqmypNrLba8rRem";
}

pub mod indexers {
    pub const ARB1_DATA_NEXUS: &str = "https://arb-service.thegraph.datanexus.tech/";
    pub const ARB1_ELLIPFRA: &str = "https://graph-l2prod.ellipfra.com/";
}

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
pub fn test_indexer_from_url(url: impl Into<String>) -> Arc<impl IndexerClient> {
    let url: Url = url.into().parse().expect("Invalid status url");

    let mut addr = url.to_string().into_bytes();
    addr.resize(20, 0);
    // Create a fake address from the URL.
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
        IntCounterVec::new(
            prometheus_exporter::prometheus::Opts::new("foo", "bar"),
            &["a", "b"],
        )
        .unwrap(),
    ))
}

/// Parses the [`IpfsCid`] of a subgraph deployment.
pub fn ipfs_cid(deployment: impl Into<String>) -> IpfsCid {
    IpfsCid::from_str(&deployment.into()).unwrap()
}

pub fn fast_rng(seed_extra: u64) -> SmallRng {
    SmallRng::seed_from_u64(*TEST_SEED + seed_extra)
}
