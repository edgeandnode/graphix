mod interceptor;
mod real_indexer;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use graphix_common_types::{BlockHash, GraphNodeCollectedVersion, IndexerAddress, PoiBytes};
pub use interceptor::IndexerInterceptor;
pub use real_indexer::RealIndexer;
use serde::Serialize;

/// An indexer is a `graph-node` instance that can be queried for information.
#[async_trait]
pub trait IndexerClient: Send + Sync + Debug {
    /// The indexer's address.
    ///
    /// If the indexer is not a network participant (i.e. it can't be found on
    /// the network subgraph), then a fake address MAY be used as long as it's
    /// guaranteed to be unique.
    fn address(&self) -> IndexerAddress;

    /// Human-readable name of the indexer.
    fn name(&self) -> Option<Cow<str>>;

    async fn ping(self: Arc<Self>) -> anyhow::Result<()>;

    async fn indexing_statuses(self: Arc<Self>) -> anyhow::Result<Vec<IndexingStatus>>;

    async fn proofs_of_indexing(self: Arc<Self>, requests: Vec<PoiRequest>)
        -> Vec<ProofOfIndexing>;

    async fn version(self: Arc<Self>) -> anyhow::Result<GraphNodeCollectedVersion>;

    async fn subgraph_api_versions(
        self: Arc<Self>,
        subgraph_id: &str,
    ) -> anyhow::Result<Vec<String>>;

    /// Convenience wrapper around calling [`IndexerClient::proofs_of_indexing`] for a
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
    fn address(&self) -> IndexerAddress;
    fn name(&self) -> Option<Cow<str>>;

    /// Returns the string representation of the indexer's address using
    /// [`HexString`].
    fn address_string(&self) -> String {
        self.address().to_string()
    }
}

impl<T> IndexerId for T
where
    T: IndexerClient,
{
    fn address(&self) -> IndexerAddress {
        IndexerClient::address(self)
    }

    fn name(&self) -> Option<Cow<str>> {
        IndexerClient::name(self)
    }
}

impl IndexerId for Arc<dyn IndexerClient> {
    fn address(&self) -> IndexerAddress {
        IndexerClient::address(&**self)
    }

    fn name(&self) -> Option<Cow<str>> {
        IndexerClient::name(&**self)
    }
}

impl PartialEq for dyn IndexerClient {
    fn eq(&self, other: &Self) -> bool {
        self.address() == other.address()
    }
}

impl Eq for dyn IndexerClient {}

impl Hash for dyn IndexerClient {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // It's best to hash addresses even though entropy is typically already
        // high, because some Graphix configurations may use human-readable
        // strings as fake addresses.
        self.address().hash(state)
    }
}

impl PartialOrd for dyn IndexerClient {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for dyn IndexerClient {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.address().cmp(&other.address())
    }
}

/// A wrapper around some inner data `T` that has an associated [`Indexer`].
pub struct WithIndexer<T> {
    pub indexer: Arc<dyn IndexerClient>,
    pub inner: T,
}

impl<T> WithIndexer<T> {
    pub fn new(indexer: Arc<dyn IndexerClient>, inner: T) -> Self {
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd)]
pub struct BlockPointer {
    pub number: u64,
    pub hash: Option<BlockHash>,
}

impl fmt::Display for BlockPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{} ({})",
            self.number,
            self.hash
                .as_ref()
                .map_or("no hash".to_string(), |hash| format!("{}", hash))
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct SubgraphDeployment(pub String);

impl Deref for SubgraphDeployment {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Eq)]
pub struct IndexingStatus {
    pub indexer: Arc<dyn IndexerClient>,
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
    pub earliest_block_num: u64,
}

impl PartialEq for IndexingStatus {
    fn eq(&self, other: &Self) -> bool {
        self.indexer.as_ref() == other.indexer.as_ref()
            && self.deployment == other.deployment
            && self.network == other.network
            && self.latest_block == other.latest_block
    }
}

#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct ProofOfIndexing {
    pub indexer: Arc<dyn IndexerClient>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: PoiBytes,
}

impl PartialEq for ProofOfIndexing {
    fn eq(&self, other: &Self) -> bool {
        self.indexer.as_ref() == other.indexer.as_ref()
            && self.deployment == other.deployment
            && self.block == other.block
            && self.proof_of_indexing == other.proof_of_indexing
    }
}

pub trait WritablePoi {
    type IndexerId: IndexerId;

    fn deployment_cid(&self) -> &str;
    fn indexer_id(&self) -> Self::IndexerId;
    fn block(&self) -> &BlockPointer;
    fn proof_of_indexing(&self) -> &PoiBytes;
}

impl WritablePoi for ProofOfIndexing {
    type IndexerId = Arc<dyn IndexerClient>;

    fn deployment_cid(&self) -> &str {
        self.deployment.as_str()
    }

    fn indexer_id(&self) -> Self::IndexerId {
        self.indexer.clone()
    }

    fn block(&self) -> &BlockPointer {
        &self.block
    }

    fn proof_of_indexing(&self) -> &PoiBytes {
        &self.proof_of_indexing
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct DivergingBlock {
    pub block: BlockPointer,
    pub proof_of_indexing1: PoiBytes,
    pub proof_of_indexing2: PoiBytes,
}

#[derive(Clone, Debug)]
pub struct POICrossCheckReport {
    pub poi1: ProofOfIndexing,
    pub poi2: ProofOfIndexing,
    pub diverging_block: Option<DivergingBlock>,
}

#[derive(Debug, Clone)]
pub struct PoiRequest {
    pub deployment: SubgraphDeployment,
    pub block_number: u64,
}
