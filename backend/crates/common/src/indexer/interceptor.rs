use std::sync::Arc;

use crate::indexer::types::Indexer;
use crate::prelude::Bytes32;
use crate::types::{IndexingStatus, POIRequest, ProofOfIndexing};
use async_trait::async_trait;

/// Pretends to be an indexer by routing requests a `RealIndexer` and then intercepting the
/// responses to generate diverging PoIs. The divergent pois will consist of a repetition of
/// `poi_bit`.
#[derive(Debug)]
pub struct IndexerInterceptor {
    target: Arc<dyn Indexer>,
    id: String,
    poi_byte: u8,
}

impl IndexerInterceptor {
    pub fn new(id: String, target: Arc<dyn Indexer>, poi_byte: u8) -> Self {
        Self {
            id,
            target,
            poi_byte,
        }
    }
}

#[async_trait]

impl Indexer for IndexerInterceptor {
    fn id(&self) -> &str {
        &self.id
    }

    fn address(&self) -> Option<&[u8]> {
        None
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
            })
            .collect();
        Ok(hijacked_statuses)
    }

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<POIRequest>,
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
}
