use tracing::*;

use crate::config::TestingConfig;
use crate::indexer::RealIndexer;

#[instrument]
pub fn testing_indexers(config: TestingConfig) -> Vec<RealIndexer> {
    config.environments.iter().map(RealIndexer::new).collect()
}
