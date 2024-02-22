use std::fmt::Debug;
use std::sync::Arc;

use async_graphql::async_trait;
use graphix_indexer_client::Indexer;
use graphix_store::Store;

#[derive(Debug, Clone)]
pub struct UnforgivingIndexerClient<I> {
    indexer: I,
    store: Store,
}

//#[async_trait]
//impl<I: Indexer + Debug> Indexer for UnforgivingIndexerClient<I> {
//    fn address(&self) -> &[u8] {
//        self.indexer.address()
//    }
//
//    fn name(&self) -> Option<std::borrow::Cow<str>> {
//        self.indexer.name()
//    }
//
//    async fn version(self: Arc<Self>) -> anyhow::Result<graphix_common_types::IndexerVersion> {
//        self.indexer.version().await
//    }
//
//    async fn ping(self: Arc<Self>) -> anyhow::Result<()> {
//        self.indexer.ping().await
//    }
//
//    async fn indexing_statuses(
//        self: Arc<Self>,
//    ) -> anyhow::Result<Vec<graphix_indexer_client::IndexingStatus>> {
//        self.store
//            .failed_query(&self.indexer, "indexing_statuses")
//            .await?;
//    }
//
//    async fn block_cache_contents(
//        self: Arc<Self>,
//        network: &str,
//        block_hash: &[u8],
//    ) -> anyhow::Result<Option<serde_json::Value>> {
//    }
//
//    async fn cached_eth_calls(
//        self: Arc<Self>,
//        network: &str,
//        block_hash: &[u8],
//    ) -> anyhow::Result<Vec<graphix_indexer_client::CachedEthereumCall>> {
//    }
//
//    async fn entity_changes(
//        self: Arc<Self>,
//        subgraph_id: &str,
//        block_number: u64,
//    ) -> anyhow::Result<graphix_indexer_client::EntityChanges> {
//    }
//
//    async fn proof_of_indexing(
//        self: Arc<Self>,
//        request: graphix_indexer_client::PoiRequest,
//    ) -> Result<graphix_indexer_client::ProofOfIndexing, anyhow::Error> {
//    }
//
//    async fn proofs_of_indexing(
//        self: Arc<Self>,
//        requests: Vec<graphix_indexer_client::PoiRequest>,
//    ) -> Vec<graphix_indexer_client::ProofOfIndexing> {
//    }
//
//    async fn subgraph_api_versions(
//        self: Arc<Self>,
//        subgraph_id: &str,
//    ) -> anyhow::Result<Vec<String>> {
//    }
//}
