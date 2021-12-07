use std::sync::Arc;
use std::time::Duration;

use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::indexer::Indexer;
use crate::types::IndexingStatus;

pub fn indexing_statuses(indexers: Eventual<Vec<Arc<Indexer>>>) -> Eventual<Vec<IndexingStatus>> {
    join((indexers, timer(Duration::from_secs(120))))
        .subscribe()
        .map(|(indexers, _)| query_indexing_statuses(indexers))
}

async fn query_indexing_statuses(indexers: Vec<Arc<Indexer>>) -> Vec<IndexingStatus> {
    info!("Query indexing statuses");

    indexers
        .iter()
        .map(|indexer| indexer.clone().indexing_statuses())
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .zip(indexers)
        .filter_map(skip_errors)
        .flatten()
        .collect()
}

fn skip_errors(
    result: (Result<Vec<IndexingStatus>, anyhow::Error>, Arc<Indexer>),
) -> Option<Vec<IndexingStatus>> {
    let url = result.1.urls.status.to_string();
    match result.0 {
        Ok(indexing_statuses) => {
            info!(id = %result.1.id, %url, "Successfully queried indexing statuses");
            Some(indexing_statuses)
        }
        Err(error) => {
            warn!(id = %result.1.id, %url, %error, "Failed to query indexing statuses");
            None
        }
    }
}
