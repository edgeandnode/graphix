use core::hash::Hash;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use crate::{
    config::IndexerUrls,
    types::{IndexingStatus, POIRequest, ProofOfIndexing},
};

#[async_trait]
pub trait Indexer: Clone + Sized + Eq + Send + Sync + Hash + Ord {
    fn id(&self) -> &String;
    fn urls(&self) -> &IndexerUrls;

    async fn indexing_statuses(self: Arc<Self>)
        -> Result<Vec<IndexingStatus<Self>>, anyhow::Error>;

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<POIRequest>,
    ) -> Result<Vec<ProofOfIndexing<Self>>, anyhow::Error>;
}
