use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::block_choice::BlockChoicePolicy;
use crate::indexer::Indexer;
use crate::types::{
    IndexerVersion, IndexingStatus, PoiRequest, ProofOfIndexing, SubgraphDeployment,
};
use crate::PrometheusMetrics;

/// Queries all `indexingStatuses` for all the given indexers.
#[instrument(skip_all)]
pub async fn query_indexing_statuses(
    indexers: Vec<Arc<dyn Indexer>>,
    metrics: &PrometheusMetrics,
) -> Vec<IndexingStatus> {
    let indexer_count = indexers.len();
    debug!(indexers = indexer_count, "Querying indexing statuses...");

    let span = span!(Level::TRACE, "query_indexing_statuses");
    let enter_span = span.enter();

    let mut futures = FuturesUnordered::new();
    for indexer in indexers {
        futures.push(async move { (indexer.clone(), indexer.indexing_statuses().await) });
    }

    let mut indexing_statuses = vec![];
    let mut query_successes = 0;
    let mut query_failures = 0;

    while let Some((indexer, query_res)) = futures.next().await {
        match query_res {
            Ok(statuses) => {
                query_successes += 1;
                metrics
                    .indexing_statuses_requests
                    .get_metric_with_label_values(&[&indexer.id(), "1"])
                    .unwrap()
                    .inc();

                debug!(
                    indexer_id = %indexer.id(),
                    statuses = %statuses.len(),
                    "Successfully queried indexing statuses"
                );
                indexing_statuses.extend(statuses);
            }

            Err(error) => {
                query_failures += 1;
                metrics
                    .indexing_statuses_requests
                    .get_metric_with_label_values(&[&indexer.id(), "0"])
                    .unwrap()
                    .inc();

                debug!(
                    indexer_id = %indexer.id(),
                    %error,
                    "Failed to query indexing statuses"
                );
            }
        }
    }

    std::mem::drop(enter_span);

    info!(
        indexers = indexer_count,
        indexing_statuses = indexing_statuses.len(),
        %query_successes,
        %query_failures,
        "Finished querying indexing statuses for all indexers"
    );

    indexing_statuses
}

/// Queries all `indexers` for their `graph-node` versions.
pub async fn query_graph_node_versions(
    indexers: &[Arc<dyn Indexer>],
    _metrics: &PrometheusMetrics,
) -> HashMap<Arc<dyn Indexer>, anyhow::Result<IndexerVersion>> {
    let span = span!(Level::TRACE, "query_graph_node_versions");
    let _enter_span = span.enter();

    info!("Querying graph-node versions...");

    let mut futures = FuturesUnordered::new();
    for indexer in indexers {
        futures.push(async move { (indexer.clone(), indexer.clone().version().await) });
    }

    let mut versions = HashMap::new();
    while let Some((indexer, version_result)) = futures.next().await {
        match &version_result {
            Ok(version) => {
                trace!(
                    indexer_id = %indexer.id(),
                    version = %version.version,
                    commit = %version.commit,
                    "Successfully queried graph-node version"
                );
            }
            Err(error) => {
                trace!(
                    indexer_id = %indexer.id(),
                    %error,
                    "Failed to query graph-node version"
                );
            }
        }
        versions.insert(indexer, version_result);
    }

    info!(
        indexers = versions.len(),
        "Finished querying graph-node versions for all indexers"
    );

    versions
}

pub async fn query_proofs_of_indexing(
    indexing_statuses: Vec<IndexingStatus>,
    block_choice_policy: BlockChoicePolicy,
) -> Vec<ProofOfIndexing> {
    info!("Query POIs for recent common blocks across indexers");

    let span = span!(Level::TRACE, "query_proofs_of_indexing");
    let _enter_span = span.enter();

    // Identify all indexers
    let indexers = indexing_statuses
        .iter()
        .map(|status| status.indexer.clone())
        .collect::<HashSet<_>>();

    // Identify all deployments
    let deployments: HashSet<SubgraphDeployment> = HashSet::from_iter(
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

    // For each deployment, chooose a block on which to query the Poi
    let latest_blocks: HashMap<SubgraphDeployment, Option<u64>> =
        HashMap::from_iter(deployments.iter().map(|deployment| {
            (
                deployment.clone(),
                statuses_by_deployment.get(deployment).and_then(|statuses| {
                    block_choice_policy.choose_block(statuses.iter().copied())
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
                        block_number,
                    })
                })
                .collect::<Vec<_>>();

            let pois = indexer.clone().proofs_of_indexing(poi_requests).await;

            debug!(
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
