use serde::Serialize;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::indexer::Indexer;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct BlockPointer {
    pub number: u64,
    pub hash: Option<Bytes32>,
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
pub struct SubgraphDeployment {
    pub deployment_id: String,
    pub network: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EntityChanges {
    pub updates: HashMap<String, Vec<serde_json::Value>>,
    pub deletions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IndexingStatus<I>
where
    I: Indexer,
{
    pub indexer: Arc<I>,
    pub deployment: SubgraphDeployment,
    pub latest_block: BlockPointer,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd)]
pub struct Bytes32(pub Vec<u8>);

impl TryFrom<&str> for Bytes32 {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self(hex::decode(s.trim_start_matches("0x"))?))
    }
}

impl fmt::Display for Bytes32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

impl Into<String> for Bytes32 {
    fn into(self: Bytes32) -> String {
        format!("{}", self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfIndexing<I>
where
    I: Indexer,
{
    pub indexer: Arc<I>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
    pub debug_data: PoiDebugData,
}

impl<I: Indexer> Ord for ProofOfIndexing<I> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl<I: Indexer> PartialOrd for ProofOfIndexing<I> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            self.indexer
                .cmp(&other.indexer)
                .then(self.deployment.cmp(&other.deployment))
                .then(self.block.number.cmp(&other.block.number)),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivergingBlock {
    pub block: BlockPointer,
    pub proof_of_indexing1: Bytes32,
    pub proof_of_indexing2: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct POICrossCheckReport<I>
where
    I: Indexer,
{
    pub poi1: ProofOfIndexing<I>,
    pub poi2: ProofOfIndexing<I>,
    pub diverging_block: Option<DivergingBlock>,
}

#[derive(Debug, Clone)]
pub struct POIRequest {
    pub deployment: SubgraphDeployment,
    pub block_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoiDebugData {
    pub entity_updates: HashMap<String, Vec<serde_json::Value>>,
    pub entity_deletions: HashMap<String, Vec<String>>,
    pub block_contents: serde_json::Value,
    pub cached_calls: Vec<CachedEthereumCall>,
}

impl PoiDebugData {
    pub fn empty() -> Self {
        Self {
            entity_updates: HashMap::new(),
            entity_deletions: HashMap::new(),
            block_contents: serde_json::Value::Null,
            cached_calls: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEthereumCall {
    pub id_hash: Vec<u8>,
    pub contract_address: Vec<u8>,
    pub return_value: Vec<u8>,
}
