//! A indexer interceptor, for test configs only.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;

use super::{CachedEthereumCall, EntityChanges};
use crate::indexer::Indexer;
use crate::prelude::Bytes32;
use crate::types::{self, IndexingStatus, PoiRequest, ProofOfIndexing};

/// Pretends to be an indexer by routing requests a
/// [`RealIndexer`](crate::indexer::RealIndexer) and then intercepting the
/// responses to generate diverging Pois. The divergent pois will consist of a
/// repetition of `poi_byte`. Interceptors have no [`Indexer::address`].
#[derive(Debug)]
pub struct IndexerInterceptor {
    target: Arc<dyn Indexer>,
    poi_byte: u8,
}

impl IndexerInterceptor {
    pub fn new(target: Arc<dyn Indexer>, poi_byte: u8) -> Self {
        Self { target, poi_byte }
    }
}

#[async_trait]

impl Indexer for IndexerInterceptor {
    fn name(&self) -> Option<Cow<'_, String>> {
        self.target
            .name()
            .map(|name| Cow::Owned(format!("interceptor-{}", name)))
    }

    fn address(&self) -> Option<&[u8]> {
        None
    }

    async fn ping(self: Arc<Self>) -> anyhow::Result<()> {
        self.target.clone().ping().await
    }

    async fn indexing_statuses(self: Arc<Self>) -> Result<Vec<IndexingStatus>, anyhow::Error> {
        let statuses = self.target.clone().indexing_statuses().await?;
        let hijacked_statuses = statuses
            .into_iter()
            .map(|status| IndexingStatus {
                indexer: self.clone(),
                deployment: status.deployment,
                network: status.network,
                latest_block: status.latest_block,
                earliest_block_num: status.earliest_block_num,
            })
            .collect();
        Ok(hijacked_statuses)
    }

    async fn version(self: Arc<Self>) -> anyhow::Result<types::IndexerVersion> {
        self.target.clone().version().await
    }

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<PoiRequest>,
    ) -> Vec<ProofOfIndexing> {
        let pois = self.target.clone().proofs_of_indexing(requests).await;

        pois.into_iter()
            .map(|poi| {
                let divergent_poi = Bytes32([self.poi_byte; 32]);
                ProofOfIndexing {
                    indexer: self.clone(),
                    deployment: poi.deployment,
                    block: poi.block,
                    proof_of_indexing: divergent_poi,
                }
            })
            .collect()
    }

    async fn subgraph_api_versions(
        self: Arc<Self>,
        subgraph_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.target.clone().subgraph_api_versions(subgraph_id).await
    }

    async fn cached_eth_calls(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Vec<CachedEthereumCall>> {
        self.target
            .clone()
            .cached_eth_calls(network, block_hash)
            .await
    }

    async fn block_cache_contents(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Option<serde_json::Value>> {
        self.target
            .clone()
            .block_cache_contents(network, block_hash)
            .await
    }

    async fn entity_changes(
        self: Arc<Self>,
        subgraph_id: &str,
        block_number: u64,
    ) -> anyhow::Result<EntityChanges> {
        self.target
            .clone()
            .entity_changes(subgraph_id, block_number)
            .await
    }
}
