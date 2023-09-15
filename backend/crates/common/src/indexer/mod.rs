mod interceptor;
mod real_indexer;

pub use interceptor::IndexerInterceptor;
pub use real_indexer::RealIndexer;

use crate::types::{IndexingStatus, PoiRequest, ProofOfIndexing};
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::{fmt::Debug, hash::Hash};

#[async_trait]
pub trait Indexer: Send + Sync + Debug {
    /// Uniquely identifies the indexer. This is relied on for the [`Hash`] and
    /// [`Eq`] impls.
    fn id(&self) -> &str;

    fn address(&self) -> Option<&[u8]>;

    async fn indexing_statuses(self: Arc<Self>) -> anyhow::Result<Vec<IndexingStatus>>;

    async fn proofs_of_indexing(self: Arc<Self>, requests: Vec<PoiRequest>)
        -> Vec<ProofOfIndexing>;

    /// Convenience wrapper around calling [`Indexer::proofs_of_indexing`] for a
    /// single POI.
    async fn proof_of_indexing(
        self: Arc<Self>,
        request: PoiRequest,
    ) -> Result<ProofOfIndexing, anyhow::Error> {
        let pois = self.proofs_of_indexing(vec![request.clone()]).await;
        match pois.len() {
            0 => return Err(anyhow!("no proof of indexing returned {:?}", request)),
            1 => return Ok(pois.into_iter().next().unwrap()),
            _ => return Err(anyhow!("multiple proofs of indexing returned")),
        }
    }

    async fn cached_eth_calls(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Vec<CachedEthereumCall>>;

    async fn block_cache_contents(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Option<serde_json::Value>>;

    async fn entity_changes(
        self: Arc<Self>,
        subgraph_id: &str,
        block_number: u64,
    ) -> anyhow::Result<EntityChanges>;
}

impl PartialEq for dyn Indexer {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for dyn Indexer {}

impl Hash for dyn Indexer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

impl PartialOrd for dyn Indexer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id().partial_cmp(other.id())
    }
}

impl Ord for dyn Indexer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

/// A wrapper around some inner data `T` that has an associated [`Indexer`].
pub struct WithIndexer<T> {
    pub indexer: Arc<dyn Indexer>,
    pub inner: T,
}

impl<T> WithIndexer<T> {
    pub fn new(indexer: Arc<dyn Indexer>, inner: T) -> Self {
        Self { indexer, inner }
    }
}

#[derive(Debug)]
pub struct CachedEthereumCall {
    pub id_hash: Vec<u8>,
    pub return_value: Vec<u8>,
    pub contract_address: Vec<u8>,
}

pub type EntityType = String;
pub type EntityId = String;

pub struct EntityChanges {
    pub updates: HashMap<EntityType, Vec<serde_json::Value>>,
    pub deletions: HashMap<EntityType, Vec<EntityId>>,
}
