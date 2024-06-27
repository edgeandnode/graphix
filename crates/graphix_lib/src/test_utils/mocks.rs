use std::borrow::Cow;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use graphix_common_types::{GraphNodeCollectedVersion, IndexerAddress, IpfsCid, PoiBytes};
use graphix_indexer_client::{
    BlockPointer, CachedEthereumCall, EntityChanges, IndexerClient, IndexingStatus, PoiRequest,
    ProofOfIndexing,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeploymentDetails {
    pub deployment: IpfsCid,
    pub network: String,
    pub latest_block: BlockPointer,
    pub canonical_pois: Vec<PartialProofOfIndexing>,
    pub earliest_block_num: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MockIndexer {
    pub name: String,
    pub deployment_details: Vec<DeploymentDetails>,
    pub fail_indexing_statuses: bool,
}

#[async_trait]
impl IndexerClient for MockIndexer {
    fn name(&self) -> Option<Cow<str>> {
        Some(Cow::Borrowed(&self.name))
    }

    fn address(&self) -> IndexerAddress {
        let mut addr = self.name.clone().into_bytes();
        addr.resize(20, 0);
        <[u8; 20]>::try_from(addr).unwrap().into()
    }

    async fn indexing_statuses(self: Arc<Self>) -> Result<Vec<IndexingStatus>, anyhow::Error> {
        if self.fail_indexing_statuses {
            Err(anyhow!("boo"))
        } else {
            Ok(self
                .deployment_details
                .clone()
                .into_iter()
                .map(|details| IndexingStatus {
                    indexer: self.clone(),
                    deployment: details.deployment,
                    network: details.network,
                    latest_block: details.latest_block,
                    earliest_block_num: details.earliest_block_num,
                })
                .collect())
        }
    }

    async fn ping(self: Arc<Self>) -> anyhow::Result<()> {
        Ok(())
    }

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<PoiRequest>,
    ) -> Vec<ProofOfIndexing> {
        // TODO: Introduce discrepancies from canonical POIs into the mix
        requests
            .into_iter()
            .filter_map(|request| {
                self.deployment_details
                    .iter()
                    .find(|detail| detail.deployment.eq(&request.deployment))
                    .map(|detail| (request, detail))
            })
            .filter_map(|(request, detail)| {
                detail
                    .canonical_pois
                    .iter()
                    .find(|poi| poi.block.number.eq(&request.block_number))
                    .map(|poi| (detail, poi))
            })
            .map(|(deployment_detail, poi)| ProofOfIndexing {
                indexer: self.clone(),
                deployment: deployment_detail.deployment.clone(),
                block: poi.block.clone(),
                proof_of_indexing: poi.proof_of_indexing,
            })
            .collect::<Vec<_>>()
    }

    async fn version(self: Arc<Self>) -> anyhow::Result<GraphNodeCollectedVersion> {
        Ok(GraphNodeCollectedVersion {
            version: Some("0.0.0".to_string()),
            commit: Some("no-commit-hash".to_string()),
            error_response: None,
            collected_at: chrono::Utc::now().naive_utc(),
        })
    }

    async fn subgraph_api_versions(
        self: Arc<Self>,
        _subgraph_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn cached_eth_calls(
        self: Arc<Self>,
        _network: &str,
        _block_hash: &[u8],
    ) -> anyhow::Result<Vec<CachedEthereumCall>> {
        Ok(vec![])
    }

    async fn block_cache_contents(
        self: Arc<Self>,
        _network: &str,
        _block_hash: &[u8],
    ) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn entity_changes(
        self: Arc<Self>,
        _subgraph_id: &str,
        _block_number: u64,
    ) -> anyhow::Result<EntityChanges> {
        Ok(EntityChanges {
            updates: Default::default(),
            deletions: Default::default(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartialProofOfIndexing {
    pub block: BlockPointer,
    pub proof_of_indexing: PoiBytes,
}
