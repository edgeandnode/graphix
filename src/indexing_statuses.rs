use std::sync::Arc;
use std::time::Duration;

use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::indexer::Indexer;
use crate::types::IndexingStatus;

pub fn indexing_statuses<T>(indexers: Eventual<Vec<Arc<T>>>) -> Eventual<Vec<IndexingStatus<T>>>
where
    T: Indexer + 'static,
{
    join((indexers, timer(Duration::from_secs(120))))
        .subscribe()
        .map(|(indexers, _)| query_indexing_statuses(indexers))
}

async fn query_indexing_statuses<T>(indexers: Vec<Arc<T>>) -> Vec<IndexingStatus<T>>
where
    T: Indexer,
{
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

fn skip_errors<T>(
    result: (Result<Vec<IndexingStatus<T>>, anyhow::Error>, Arc<T>),
) -> Option<Vec<IndexingStatus<T>>>
where
    T: Indexer,
{
    let url = result.1.urls().status.to_string();
    match result.0 {
        Ok(indexing_statuses) => {
            info!(id = %result.1.id(), %url, statuses=%indexing_statuses.len(), "Successfully queried indexing statuses");
            Some(indexing_statuses)
        }
        Err(error) => {
            warn!(id = %result.1.id(), %url, %error, "Failed to query indexing statuses");
            None
        }
    }
}
