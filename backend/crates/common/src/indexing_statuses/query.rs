use std::sync::Arc;

use crate::prelude::{Indexer, IndexingStatus};
use crate::prometheus_metrics::metrics;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
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
