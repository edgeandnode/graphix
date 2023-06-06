use std::sync::Arc;

use tracing::*;

use crate::config::TestingConfig;
use crate::indexer::interceptor::IndexerInterceptor;
use crate::indexer::{Indexer, RealIndexer};
use crate::prelude::EnvironmentConfig;

#[instrument]
pub fn testing_indexers(config: TestingConfig) -> Vec<Arc<dyn Indexer>> {
    let mut indexers: Vec<Arc<dyn Indexer>> = vec![];

    // First, configure all the real indexers.
    for config in &config.environments {
        match config {
            EnvironmentConfig::Indexer(config) => {
                info!(indexer_id = %config.id, "Configuring indexer");
                indexers.push(Arc::new(RealIndexer::new(config.clone())));
            }
            EnvironmentConfig::Interceptor(_) => {}
        }
    }

    // Then, configure all the interceptors, referring to the real indexers by ID.
    for config in config.environments {
        match config {
            EnvironmentConfig::Indexer(_config) => {}
            EnvironmentConfig::Interceptor(config) => {
                info!(interceptor_id = %config.id, "Configuring interceptor");
                let target = indexers
                    .iter()
                    .find(|indexer| indexer.id() == config.target)
                    .expect("interceptor target indexer not found");
                indexers.push(Arc::new(IndexerInterceptor::new(
                    config.id,
                    target.clone(),
                    config.poi_byte,
                )));
            }
        }
    }

    indexers
}
