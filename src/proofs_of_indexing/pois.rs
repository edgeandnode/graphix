use std::collections::{hash_map::RandomState, HashMap, HashSet};

use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::{
    indexer::{Indexer, POIRequest},
    indexing_statuses::IndexerWithIndexingStatuses,
    types::{BlockPointer, Bytes32, SubgraphDeployment},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfIndexing {
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerWithProofsOfIndexing {
    pub indexer: IndexerWithIndexingStatuses,
    pub proofs_of_indexing: Vec<ProofOfIndexing>,
}

pub fn proofs_of_indexing(
    indexers: Eventual<Vec<IndexerWithIndexingStatuses>>,
) -> Eventual<Vec<IndexerWithProofsOfIndexing>> {
    indexers.map(query_proofs_of_indexing)
}

async fn query_proofs_of_indexing(
    indexers: Vec<IndexerWithIndexingStatuses>,
) -> Vec<IndexerWithProofsOfIndexing> {
    info!("Query POIs for recent common blocks across indexers");

    // Obtain a flat iterator over all indexing statuses
    let indexing_statuses = indexers
        .iter()
        .map(|indexer| indexer.indexing_statuses.clone())
        .flatten()
        .collect::<Vec<_>>();

    // Identify all deployments across the indexers
    let deployments: HashSet<SubgraphDeployment, RandomState> = HashSet::from_iter(
        indexing_statuses
            .clone()
            .into_iter()
            .map(|status| status.deployment),
    );

    // Identify which indexers have which deployments
    let deployment_indexers: HashMap<SubgraphDeployment, Vec<IndexerWithIndexingStatuses>> =
        HashMap::from_iter(deployments.iter().map(|deployment| {
            (
                deployment.clone(),
                indexers
                    .iter()
                    .filter(|indexer| {
                        indexer
                            .indexing_statuses
                            .iter()
                            .find(|status| status.deployment.eq(deployment))
                            .is_some()
                    })
                    .cloned()
                    .collect(),
            )
        }));

    // For each deployment, identify the latest block number that all indexers have in common
    let latest_blocks: HashMap<SubgraphDeployment, Option<BlockPointer>> =
        HashMap::from_iter(deployments.iter().map(move |deployment| {
            (
                deployment.clone(),
                indexing_statuses
                    .iter()
                    .filter(|status| status.deployment.eq(deployment))
                    .map(|status| status.latest_block.clone())
                    .min_by_key(|block| block.number),
            )
        }));

    // Fetch POIs for the most recent common blocks
    let poi_results = indexers
        .iter()
        .map(|indexer| {
            let poi_requests = latest_blocks
                .iter()
                .filter(|(deployment, _)| {
                    deployment_indexers
                        .get(*deployment)
                        .expect("bug in matching deployments to latest blocks and indexers")
                        .contains(&indexer)
                })
                .filter_map(|(deployment, block)| {
                    block.clone().map(|block| POIRequest {
                        deployment: deployment.clone(),
                        block,
                    })
                })
                .collect::<Vec<_>>();

            indexer.indexer.proofs_of_indexing(poi_requests)
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .zip(indexers.into_iter())
        .into_iter()
        .filter_map(skip_errors)
        .collect::<Vec<_>>();

    poi_results
        .into_iter()
        .map(
            |(indexer, proofs_of_indexing)| IndexerWithProofsOfIndexing {
                indexer,
                proofs_of_indexing,
            },
        )
        .collect()
}

fn skip_errors(
    result: (
        Result<Vec<ProofOfIndexing>, anyhow::Error>,
        IndexerWithIndexingStatuses,
    ),
) -> Option<(IndexerWithIndexingStatuses, Vec<ProofOfIndexing>)> {
    let Indexer { id, urls, .. } = &result.1.indexer;
    let url = urls.status.to_string();
    match result.0 {
        Ok(poi_infos) => {
            debug!(%id, %url, "Successfully queried POIs from indexer");
            Some((result.1, poi_infos))
        }
        Err(error) => {
            warn!(%id, %url, %error, "Failed to query POIs from indexer");
            None
        }
    }
}
