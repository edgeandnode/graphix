use graphix_common::prelude::{
    DivergingBlock as DivergentBlock, PoiRequest, ProofOfIndexing, SubgraphDeployment,
};
use tracing::{debug, info};

pub struct DivergingBlock {
    pub poi1: ProofOfIndexing,
    pub poi2: ProofOfIndexing,
}

impl From<DivergingBlock> for DivergentBlock {
    fn from(other: DivergingBlock) -> DivergentBlock {
        Self {
            block: other.poi1.block,
            proof_of_indexing1: other.poi1.proof_of_indexing,
            proof_of_indexing2: other.poi2.proof_of_indexing,
        }
    }
}

#[derive(Clone)]
pub struct PoiBisectingContext {
    bisection_id: String,
    poi1: ProofOfIndexing,
    poi2: ProofOfIndexing,
    deployment: SubgraphDeployment,
}

impl PoiBisectingContext {
    pub fn new(
        bisection_id: String,
        poi1: ProofOfIndexing,
        poi2: ProofOfIndexing,
        deployment: SubgraphDeployment,
    ) -> Self {
        // Before attempting to bisect PoIs, we need to make sure that the PoIs refer to:
        // 1. the same subgraph deployment, and
        // 2. the same block.
        assert_eq!(poi1.deployment, poi2.deployment);
        assert_eq!(poi1.block, poi2.block);
        // Let's also check block hashes are present (and identical, by extension).
        assert!(poi1.block.hash.is_some());
        assert!(poi2.block.hash.is_some());

        assert_ne!(poi1.proof_of_indexing, poi2.proof_of_indexing);
        assert_ne!(poi1.indexer.address(), poi2.indexer.address());

        Self {
            bisection_id,
            poi1,
            poi2,
            deployment,
        }
    }

    pub async fn start(self) -> anyhow::Result<u64> {
        let indexer1 = self.poi1.indexer;
        let indexer2 = self.poi2.indexer;
        let deployment = self.deployment;

        info!(
            bisection_id = self.bisection_id,
            deployment = deployment.as_str(),
            "Starting PoI bisecting"
        );

        // The range of block numbers that we're investigating is bounded
        // inclusively both below and above. The bisection algorithm will
        // continue searching until only a single block number is left in the
        // range.
        let mut bounds = 0..=self.poi1.block.number;

        loop {
            let block_number = (bounds.start() + bounds.end()) / 2;

            debug!(
                bisection_id = self.bisection_id.clone(),
                deployment = deployment.as_str(),
                lower_bound = ?bounds.start(),
                upper_bound = ?bounds.end(),
                block_number,
                "Bisecting PoIs"
            );

            let poi1_fut = indexer1.clone().proof_of_indexing(PoiRequest {
                deployment: deployment.clone(),
                block_number,
            });
            let poi2_fut = indexer2.clone().proof_of_indexing(PoiRequest {
                deployment: deployment.clone(),
                block_number,
            });

            let (poi1, poi2) = futures::try_join!(poi1_fut, poi2_fut)?;
            if poi1 == poi2 {
                bounds = block_number..=*bounds.end();
            } else {
                bounds = *bounds.start()..=block_number;
            }

            if bounds.start() == bounds.end() {
                break;
            }
        }

        let diverging_block = *bounds.start();
        Ok(diverging_block)
    }
}
