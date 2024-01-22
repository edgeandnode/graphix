mod interceptor;
mod real_indexer;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
pub use interceptor::IndexerInterceptor;
pub use real_indexer::RealIndexer;

use crate::types::{self, IndexingStatus, PoiRequest, ProofOfIndexing};

/// An indexer is a `graph-node` instance that may or may not also be a network
/// participant.
#[async_trait]
pub trait Indexer: Send + Sync + Debug {
    /// The indexer's address. This can be [`None`] if the indexer is not a
    /// network participant but e.g. a local `graph-node` instance.
    fn address(&self) -> Option<&[u8]>;

    /// Human-readable name of the indexer.
    fn name(&self) -> Option<Cow<'_, String>>;

    /// Within Graphix, an indexer is uniquely identified by its hex-encoded
    /// address (if it is a network participant), or its name (if it's not).
    /// This is known as the indexer's "global ID" or "GID".
    fn id(&self) -> String {
        if let Some(address) = self.address() {
            hex::encode(address)
        } else {
            self.name()
                .expect("indexer has neither address nor name; this is a bug")
                .to_string()
        }
    }

    async fn ping(self: Arc<Self>) -> anyhow::Result<()>;

    async fn indexing_statuses(self: Arc<Self>) -> anyhow::Result<Vec<IndexingStatus>>;

    async fn proofs_of_indexing(self: Arc<Self>, requests: Vec<PoiRequest>)
        -> Vec<ProofOfIndexing>;

    async fn version(self: Arc<Self>) -> anyhow::Result<types::IndexerVersion>;

    async fn subgraph_api_versions(
        self: Arc<Self>,
        subgraph_id: &str,
    ) -> anyhow::Result<Vec<String>>;

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

    /// Returns the cached Ethereum calls for the given block hash.
    async fn cached_eth_calls(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Vec<CachedEthereumCall>>;

    /// Returns the block cache contents for the given block hash.
    async fn block_cache_contents(
        self: Arc<Self>,
        network: &str,
        block_hash: &[u8],
    ) -> anyhow::Result<Option<serde_json::Value>>;

    /// Returns the entity changes for the given block number.
    async fn entity_changes(
        self: Arc<Self>,
        subgraph_id: &str,
        block_number: u64,
    ) -> anyhow::Result<EntityChanges>;
}

/// Graphix defines an indexer's ID as either its Ethereum address (if it has
/// one) or its name (if it doesn't have an address i.e. it's not a network
/// participant), strictly in this order.
pub trait IndexerId {
    fn address(&self) -> Option<&[u8]>;
    fn name(&self) -> Option<Cow<String>>;

    fn id(&self) -> String {
        if let Some(address) = self.address() {
            format!("0x{}", hex::encode(address))
        } else if let Some(name) = self.name() {
            name.to_string()
        } else {
            panic!("Indexer has neither name nor address")
        }
    }
}

impl<T> IndexerId for T
where
    T: Indexer,
{
    fn address(&self) -> Option<&[u8]> {
        Indexer::address(self)
    }

    fn name(&self) -> Option<Cow<'_, String>> {
        Indexer::name(self)
    }
}

impl IndexerId for Arc<dyn Indexer> {
    fn address(&self) -> Option<&[u8]> {
        Indexer::address(&**self)
    }

    fn name(&self) -> Option<Cow<'_, String>> {
        Indexer::name(&**self)
    }
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
        Some(self.cmp(other))
    }
}

impl Ord for dyn Indexer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(&other.id())
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
