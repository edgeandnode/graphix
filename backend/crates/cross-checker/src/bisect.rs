use std::sync::Arc;
use std::time::Duration;

use graphix_common::graphql_api::types::DivergenceInvestigationRequest;
use graphix_common::prelude::{
    BlockPointer, DivergingBlock as DivergentBlock, Indexer, PoiRequest, ProofOfIndexing,
    SubgraphDeployment,
};
use graphix_common::store::Store;
use thiserror::Error;
use tokio::sync::watch;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::utils::unordered_pairs_combinations;

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
    ) -> anyhow::Result<Self> {
        // Before attempting to bisect Pois, we need to make sure that the Pois refer to:
        // 1. the same subgraph deployment, and
        // 2. the same block.

        anyhow::ensure!(poi1.deployment == poi2.deployment);
        anyhow::ensure!(poi1.block == poi2.block);
        // FIXME!
        // Let's also check block hashes are present (and identical, by extension).
        //anyhow::ensure!(poi1.block.hash.is_some());
        //anyhow::ensure!(poi2.block.hash.is_some());
        //anyhow::ensure!(poi1.proof_of_indexing != poi2.proof_of_indexing);
        //anyhow::ensure!(poi1.indexer.address() != poi2.indexer.address());

        Ok(Self {
            bisection_id,
            poi1,
            poi2,
            deployment,
        })
    }

    pub async fn start(self) -> anyhow::Result<u64> {
        let indexer1 = self.poi1.indexer;
        let indexer2 = self.poi2.indexer;
        let deployment = self.deployment;

        info!(
            bisection_id = self.bisection_id,
            deployment = deployment.as_str(),
            "Starting Poi bisecting"
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
                "Bisecting Pois"
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

#[derive(Debug, Error)]
pub enum DivergenceInvestigationError {
    #[error("Too many POIs in a single request, the max. is {max}")]
    TooManyPois { max: u32 },
    #[error("No indexer(s) that produced the given Poi were found in the Graphix database")]
    IndexerNotFound { poi: String },
    #[error(
        "The two Pois were produced by the same indexer ({indexer_id}), bisecting the difference is not possible"
    )]
    SameIndexer { indexer_id: String },
    #[error("The two Pois were produced for different deployments, they cannot be compared: {poi1}: {poi1_deployment}, {poi2}: {poi2_deployment}")]
    DifferentDeployments {
        poi1: String,
        poi2: String,
        poi1_deployment: String,
        poi2_deployment: String,
    },
    #[error("The two Pois were produced for different blocks, they cannot be compared: {poi1}: {poi1_block}, {poi2}: {poi2_block}")]
    DifferentBlocks {
        poi1: String,
        poi2: String,
        poi1_block: i64,
        poi2_block: i64,
    },
    #[error(transparent)]
    Database(anyhow::Error),
}

pub async fn handle_divergence_investigation_requests(
    store: &Store,
    indexers: watch::Receiver<Vec<Arc<dyn Indexer>>>,
) -> anyhow::Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        debug!("Checking for new divergence investigation requests");
        let (req_uuid, req_contents_blob) =
            if let Some(x) = store.get_first_pending_divergence_investigation_request()? {
                x
            } else {
                continue;
            };

        let req_contents =
            serde_json::from_value(req_contents_blob).expect("invalid request blob; this is a bug");
        info!(req_uuid, "Found new divergence investigation request");
        let res = handle_divergence_investigation_request(
            store,
            &req_uuid,
            req_contents,
            indexers.clone(),
        )
        .await;
        if let Err(err) = res {
            error!(error = %err, "Failed to handle bisect request");
        }
        store.delete_divergence_investigation_request(&req_uuid)?;
    }
}

async fn handle_divergence_investigation_request_pair(
    store: &Store,
    indexers: &[Arc<dyn Indexer>],
    req_uuid_str: &str,
    poi1_s: &str,
    poi2_s: &str,
) -> Result<(), DivergenceInvestigationError> {
    debug!(req_uuid = req_uuid_str, poi1 = %poi1_s, poi2 = %poi2_s, "Bisecting Pois");

    let poi1 = store
        .poi(&poi1_s)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|poi_opt| {
            if let Some(poi) = poi_opt {
                Ok(poi)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi1_s.to_string(),
                })
            }
        })?;
    let poi2 = store
        .poi(&poi2_s)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|poi_opt| {
            if let Some(poi) = poi_opt {
                Ok(poi)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi2_s.to_string(),
                })
            }
        })?;

    if poi1.sg_deployment.cid != poi2.sg_deployment.cid {
        return Err(DivergenceInvestigationError::DifferentDeployments {
            poi1: poi1_s.to_string(),
            poi2: poi2_s.to_string(),
            poi1_deployment: poi1.sg_deployment.cid.to_string(),
            poi2_deployment: poi2.sg_deployment.cid.to_string(),
        }
        .into());
    }

    if poi1.block.number != poi2.block.number {
        return Err(DivergenceInvestigationError::DifferentBlocks {
            poi1: poi1_s.to_string(),
            poi2: poi2_s.to_string(),
            poi1_block: poi1.block.number,
            poi2_block: poi2.block.number,
        }
        .into());
    }

    let deployment = SubgraphDeployment(poi1.sg_deployment.cid);
    let block = BlockPointer {
        number: poi1.block.number as _,
        hash: None,
    };

    let indexer1 = indexers
        .iter()
        .find(|indexer| indexer.address() == poi1.indexer.address.as_deref())
        .cloned()
        .ok_or(DivergenceInvestigationError::IndexerNotFound {
            poi: poi1_s.to_string(),
        })?;
    let indexer2 = indexers
        .iter()
        .find(|indexer| indexer.address() == poi2.indexer.address.as_deref())
        .cloned()
        .ok_or(DivergenceInvestigationError::IndexerNotFound {
            poi: poi2_s.to_string(),
        })?;

    if indexer1.id() == indexer2.id() {
        return Err(DivergenceInvestigationError::SameIndexer {
            indexer_id: indexer1.id().to_string(),
        }
        .into());
    }

    let bisection_uuid = Uuid::new_v4().to_string();

    let poi1 = ProofOfIndexing {
        indexer: indexer1.clone(),
        deployment: deployment.clone(),
        block,
        proof_of_indexing: poi1.poi.try_into().expect("poi1 conversion failed"),
    };
    let poi2 = ProofOfIndexing {
        indexer: indexer2.clone(),
        deployment: deployment.clone(),
        block,
        proof_of_indexing: poi2.poi.try_into().expect("poi2 conversion failed"),
    };

    let context = PoiBisectingContext::new(bisection_uuid, poi1, poi2, deployment.clone())
        .expect("bisect context creation failed");
    context.start().await.expect("bisect failed");

    Ok(())
}

async fn handle_divergence_investigation_request(
    store: &Store,
    req_uuid_str: &str,
    req_contents: DivergenceInvestigationRequest,
    indexers: watch::Receiver<Vec<Arc<dyn Indexer>>>,
) -> Result<(), DivergenceInvestigationError> {
    // The number of bisections is quadratic to the number of Pois, so it's
    // important not to allow too many in a single request.
    const MAX_NUMBER_OF_POIS_PER_REQUEST: u32 = 4;

    if req_contents.pois.len() > MAX_NUMBER_OF_POIS_PER_REQUEST as usize {
        return Err(DivergenceInvestigationError::TooManyPois {
            max: MAX_NUMBER_OF_POIS_PER_REQUEST,
        }
        .into());
    }

    let indexers = indexers.borrow().clone();

    let poi_pairs = unordered_pairs_combinations(req_contents.pois.into_iter());

    for (poi1_s, poi2_s) in poi_pairs.into_iter() {
        if let Err(e) = handle_divergence_investigation_request_pair(
            store,
            &indexers,
            req_uuid_str,
            &poi1_s,
            &poi2_s,
        )
        .await
        {
            error!(req_uuid_str, error = %e, "Failed to bisect Pois");
        }
    }

    info!(req_uuid = req_uuid_str, "Finished bisecting Pois");

    Ok(())
}
