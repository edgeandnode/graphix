use core::hash::Hash;

use anyhow::anyhow;
use async_trait::async_trait;

use crate::{
    types::{IndexingStatus, POIRequest, ProofOfIndexing},
    PrometheusMetrics,
};

#[async_trait]
pub trait Indexer: Clone + Sized + Eq + Send + Sync + Hash + Ord + Send + Sync {
    fn id(&self) -> &str;
    fn address(&self) -> Option<&[u8]>;

    async fn indexing_statuses(self) -> Result<Vec<IndexingStatus<Self>>, anyhow::Error>;

    async fn proofs_of_indexing(
        self,
        metrics: &PrometheusMetrics,
        requests: Vec<POIRequest>,
    ) -> Result<Vec<ProofOfIndexing<Self>>, anyhow::Error>;

    /// Convenience wrapper around calling `proofs_of_indexing` for a single POI.
    async fn proof_of_indexing(
        self,
        metrics: &PrometheusMetrics,
        request: POIRequest,
    ) -> Result<ProofOfIndexing<Self>, anyhow::Error> {
        let mut results = self.proofs_of_indexing(metrics, vec![request]).await?;
        results
            .pop()
            .ok_or_else(|| anyhow!("no proof of indexing returned"))
    }
}
