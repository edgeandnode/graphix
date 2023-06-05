use std::sync::Arc;

use crate::{
    prelude::{
        BlockPointer, Bytes32, Indexer, IndexingStatus, POIRequest, ProofOfIndexing,
        SubgraphDeployment,
    },
    PrometheusMetrics,
};
use anyhow::anyhow;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeploymentDetails {
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
    pub canonical_pois: Vec<PartialProofOfIndexing>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MockIndexer {
    pub id: String,
    pub deployment_details: Vec<DeploymentDetails>,
    pub fail_indexing_statuses: bool,
    pub fail_proofs_of_indexing: bool,
}

#[async_trait]
impl Indexer for MockIndexer {
    fn id(&self) -> &str {
        &self.id
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
                })
                .collect())
        }
    }

    async fn proofs_of_indexing(
        self: Arc<Self>,
        _metrics: &PrometheusMetrics,
        requests: Vec<POIRequest>,
    ) -> Result<Vec<ProofOfIndexing>, anyhow::Error> {
        if self.fail_proofs_of_indexing {
            Err(anyhow!("boo"))
        } else {
            // TODO: Introduce discrepancies from canonical POIs into the mix
            Ok(requests
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
                .collect::<Vec<_>>())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartialProofOfIndexing {
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}
