use crate::prelude::{Indexer, IndexingStatus};
use crate::PrometheusMetrics;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

/// Queries all `indexingStatuses` for all `indexers`.
#[instrument(skip(_metrics, indexers))]
pub async fn query_indexing_statuses<I>(
    _metrics: &PrometheusMetrics, // TODO: use metrics
    indexers: Vec<I>,
) -> Vec<IndexingStatus<I>>
where
    I: Indexer,
{
    info!(indexers = indexers.len(), "Querying indexing statuses...");

    let mut futures = FuturesUnordered::new();
    for indexer in indexers.clone() {
        futures.push(async move { (indexer.clone(), indexer.indexing_statuses().await) });
    }

    let mut indexing_statuses = vec![];
    let mut query_successes = 0;
    let mut query_failures = 0;

    while let Some((indexer, query_res)) = futures.next().await {
        if query_res.is_ok() {
            query_successes += 1;
        } else {
            query_failures += 1;
        }

        match query_res {
            Ok(statuses) => {
                debug!(
                    indexer_id = %indexer.id(),
                    indexer_url = %indexer.urls().status,
                    statuses = %statuses.len(),
                    "Successfully queried indexing statuses"
                );
                indexing_statuses.extend(statuses);
            }

            Err(error) => {
                warn!(
                    indexer_id = %indexer.id(),
                    indexer_url = %indexer.urls().status,
                    %error,
                    "Failed to query indexing statuses"
                );
            }
        }
    }

    info!(
        indexers = indexers.len(),
        indexing_statuses = indexing_statuses.len(),
        %query_successes,
        %query_failures,
        "Finished querying indexing statuses for all indexers"
    );

    indexing_statuses
}
