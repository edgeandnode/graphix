use std::{
    collections::{hash_map::RandomState, HashMap, HashSet},
    sync::Arc,
};

use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::{
    indexer::{Indexer, POIRequest},
    types::{BlockPointer, Bytes32, IndexingStatus, SubgraphDeployment},
};

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ProofOfIndexing {
    pub indexer: Arc<Indexer>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}

pub fn proofs_of_indexing(
    indexers: Eventual<Vec<IndexingStatus>>,
) -> Eventual<Vec<ProofOfIndexing>> {
    indexers.map(query_proofs_of_indexing)
}

async fn query_proofs_of_indexing(indexing_statuses: Vec<IndexingStatus>) -> Vec<ProofOfIndexing> {
    info!("Query POIs for recent common blocks across indexers");

    // Identify all indexers
    let indexers = indexing_statuses
        .iter()
        .map(|status| status.indexer.clone())
        .collect::<HashSet<Arc<Indexer>, RandomState>>();
    // Identify all deployments
    let deployments: HashSet<SubgraphDeployment, RandomState> = HashSet::from_iter(
        indexing_statuses
            .iter()
            .map(|status| status.deployment.clone()),
    );

    // Group indexing statuses by deployment
    let statuses_by_deployment: HashMap<SubgraphDeployment, Vec<&IndexingStatus>> =
        HashMap::from_iter(deployments.iter().map(|deployment| {
            (
                deployment.clone(),
                indexing_statuses
                    .iter()
                    .filter(|status| status.deployment.eq(deployment))
                    .collect(),
            )
        }));

    // For each deployment, identify the latest block number that all indexers have in common
    let latest_blocks: HashMap<SubgraphDeployment, Option<BlockPointer>> =
        HashMap::from_iter(deployments.iter().map(|deployment| {
            (
                deployment.clone(),
                statuses_by_deployment
                    .get(deployment)
                    .map_or(None, |statuses| {
                        statuses
                            .iter()
                            .map(|status| &status.latest_block)
                            .min_by_key(|block| block.number)
                            .map(|block| block.clone())
                    }),
            )
        }));

    // Fetch POIs for the most recent common blocks
    indexers
        .iter()
        .map(|indexer| {
            let poi_requests = latest_blocks
                .iter()
                .filter(|(deployment, _)| {
                    statuses_by_deployment
                        .get(*deployment)
                        .expect("bug in matching deployments to latest blocks and indexers")
                        .iter()
                        .any(|status| status.indexer.eq(&indexer))
                })
                .filter_map(|(deployment, block)| {
                    block.clone().map(|block| POIRequest {
                        deployment: deployment.clone(),
                        block,
                    })
                })
                .collect::<Vec<_>>();

            indexer.clone().proofs_of_indexing(poi_requests)
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .zip(indexers.into_iter())
        .into_iter()
        .filter_map(skip_errors)
        .flatten()
        .collect::<Vec<_>>()
}

fn skip_errors(
    result: (Result<Vec<ProofOfIndexing>, anyhow::Error>, Arc<Indexer>),
) -> Option<Vec<ProofOfIndexing>> {
    let url = result.1.urls.status.to_string();
    match result.0 {
        Ok(pois) => {
            debug!(id = %result.1.id, %url, "Successfully queried POIs from indexer");
            Some(pois)
        }
        Err(error) => {
            warn!(id = %result.1.id, %url, %error, "Failed to query POIs from indexer");
            None
        }
    }
}
