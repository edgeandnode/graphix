use std::{borrow::Cow, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;

use crate::prelude::{
    BlockPointer, Bytes32, CachedEthereumCall, EntityChanges, Indexer, IndexerVersion,
    IndexingStatus, PoiRequest, ProofOfIndexing, SubgraphDeployment,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeploymentDetails {
    pub deployment: SubgraphDeployment,
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
impl Indexer for MockIndexer {
    fn name(&self) -> Option<Cow<String>> {
        Some(Cow::Borrowed(&self.name))
    }

    fn address(&self) -> Option<&[u8]> {
        None
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
                proof_of_indexing: poi.proof_of_indexing.clone(),
            })
            .collect::<Vec<_>>()
    }

    async fn version(self: Arc<Self>) -> anyhow::Result<IndexerVersion> {
        Ok(IndexerVersion {
            version: "0.0.0".to_string(),
            commit: "no-commit-hash".to_string(),
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
    pub proof_of_indexing: Bytes32,
}
