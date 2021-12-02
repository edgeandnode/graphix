use std::time::Duration;

use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::*;

use crate::indexer::Indexer;
use crate::types::IndexingStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerWithIndexingStatuses {
    pub indexer: Indexer,
    pub indexing_statuses: Vec<IndexingStatus>,
}

pub fn indexing_statuses(
    indexers: Eventual<Vec<Indexer>>,
) -> Eventual<Vec<IndexerWithIndexingStatuses>> {
    join((indexers, timer(Duration::from_secs(120))))
        .subscribe()
        .map(|(indexers, _)| query_indexing_statuses(indexers))
}

async fn query_indexing_statuses(indexers: Vec<Indexer>) -> Vec<IndexerWithIndexingStatuses> {
    info!("Query indexing statuses");

    indexers
        .iter()
        .map(|indexer| indexer.indexing_statuses())
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .zip(indexers)
        .filter_map(skip_errors)
        .map(|(indexer, indexing_statuses)| IndexerWithIndexingStatuses {
            indexer,
            indexing_statuses,
        })
        .collect()
}

fn skip_errors(
    result: (Result<Vec<IndexingStatus>, anyhow::Error>, Indexer),
) -> Option<(Indexer, Vec<IndexingStatus>)> {
    let Indexer { id, urls, .. } = &result.1;
    let url = urls.status.to_string();
    match result.0 {
        Ok(statuses) => {
            info!(%id, %url, "Successfully queried indexing statuses");
            Some((result.1, statuses))
        }
        Err(error) => {
            warn!(%id, %url, %error, "Failed to query indexing statuses");
            None
        }
    }
}
