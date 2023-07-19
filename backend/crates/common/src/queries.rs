use crate::block_choice::BlockChoicePolicy;
use crate::indexer::Indexer;
use crate::prelude::{IndexingStatus, PoiRequest, ProofOfIndexing, SubgraphDeployment};
use crate::prometheus_metrics::metrics;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::collections::{hash_map::RandomState, HashMap, HashSet};
use std::sync::Arc;
use tracing::*;

/// Queries all `indexingStatuses` for all `indexers`.
#[instrument(skip_all)]
pub async fn query_indexing_statuses(indexers: Vec<Arc<dyn Indexer>>) -> Vec<IndexingStatus> {
    let indexer_count = indexers.len();
    info!(indexers = indexer_count, "Querying indexing statuses...");

    let mut futures = FuturesUnordered::new();
    for indexer in indexers {
        futures.push(async move { (indexer.clone(), indexer.indexing_statuses().await) });
    }

    let mut indexing_statuses = vec![];
    let mut query_successes = 0;
    let mut query_failures = 0;

    while let Some((indexer, query_res)) = futures.next().await {
        if query_res.is_ok() {
            query_successes += 1;
            metrics()
                .indexing_statuses_requests
                .get_metric_with_label_values(&[indexer.id(), "1"])
                .unwrap()
                .inc();
        } else {
            query_failures += 1;
            metrics()
                .indexing_statuses_requests
                .get_metric_with_label_values(&[indexer.id(), "0"])
                .unwrap()
                .inc();
        }

        match query_res {
            Ok(statuses) => {
                debug!(
                    indexer_id = %indexer.id(),
                    statuses = %statuses.len(),
                    "Successfully queried indexing statuses"
                );
                indexing_statuses.extend(statuses);
            }

            Err(error) => {
                warn!(
                    indexer_id = %indexer.id(),
                    %error,
                    "Failed to query indexing statuses"
                );
            }
        }
    }

    info!(
        indexers = indexer_count,
        indexing_statuses = indexing_statuses.len(),
        %query_successes,
        %query_failures,
        "Finished querying indexing statuses for all indexers"
    );

    indexing_statuses
}

pub async fn query_proofs_of_indexing(
    indexing_statuses: Vec<IndexingStatus>,
    block_choice_policy: BlockChoicePolicy,
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

    // For each deployment, chooose a block on which to query the PoI
    let latest_blocks: HashMap<SubgraphDeployment, Option<u64>> =
        HashMap::from_iter(deployments.iter().map(|deployment| {
            (
                deployment.clone(),
                statuses_by_deployment.get(deployment).and_then(|statuses| {
                    block_choice_policy.choose_block(statuses.iter().map(|&s| s))
                }),
            )
        }));

    // Fetch POIs for the most recent common blocks
    indexers
        .iter()
        .map(|indexer| async {
            let poi_requests = latest_blocks
                .iter()
                .filter(|(deployment, &block_number)| {
                    statuses_by_deployment
                        .get(*deployment)
                        .expect("bug in matching deployments to latest blocks and indexers")
                        .iter()
                        .any(|status| {
                            status.indexer.eq(indexer)
                                && Some(status.latest_block.number) >= block_number
                        })
                })
                .filter_map(|(deployment, block_number)| {
                    block_number.map(|block_number| PoiRequest {
                        deployment: deployment.clone(),
                        block_number: block_number,
                    })
                })
                .collect::<Vec<_>>();

            let pois = indexer.clone().proofs_of_indexing(poi_requests).await;

            info!(
                id = %indexer.id(), pois = %pois.len(),
                "Successfully queried POIs from indexer"
            );

            pois
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
}
