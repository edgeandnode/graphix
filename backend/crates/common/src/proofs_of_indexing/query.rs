use std::collections::{hash_map::RandomState, HashMap, HashSet};
use std::sync::Arc;

use crate::prelude::{
    BlockPointer, Indexer, IndexingStatus, POIRequest, ProofOfIndexing, SubgraphDeployment,
};
use crate::PrometheusMetrics;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

pub async fn query_proofs_of_indexing(
    metrics: &PrometheusMetrics,
    indexing_statuses: Vec<IndexingStatus>,
) -> Vec<ProofOfIndexing> {
    info!("Query POIs for recent common blocks across indexers");

    // Identify all indexers
    let indexers = indexing_statuses
        .iter()
        .map(|status| status.indexer.clone())
        .collect::<HashSet<_>>();

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
                statuses_by_deployment.get(deployment).and_then(|statuses| {
                    statuses
                        .iter()
                        .map(|status| &status.latest_block)
                        .min_by_key(|block| block.number)
                        .cloned()
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
                        .any(|status| status.indexer.eq(indexer))
                })
                .filter_map(|(deployment, block)| {
                    block.clone().map(|block| POIRequest {
                        deployment: deployment.clone(),
                        block_number: block.number,
                    })
                })
                .collect::<Vec<_>>();

            indexer.clone().proofs_of_indexing(&metrics, poi_requests)
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
    result: (
        Result<Vec<ProofOfIndexing>, anyhow::Error>,
        Arc<dyn Indexer>,
    ),
) -> Option<Vec<ProofOfIndexing>> {
    match result.0 {
        Ok(pois) => {
            info!(
                id = %result.1.id(), pois = %pois.len(),
                "Successfully queried POIs from indexer"
            );
            Some(pois)
        }
        Err(error) => {
            warn!(
                id = %result.1.id(), %error,
                "Failed to query POIs from indexer"
            );
            None
        }
    }
}
