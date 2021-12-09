use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref, sync::Arc};

use crate::indexer::Indexer;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Ord, PartialOrd)]
pub struct BlockPointer {
    pub number: u64,
    pub hash: Bytes32,
}

impl fmt::Display for BlockPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} ({})", self.number, self.hash)
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexingStatus<T>
where
    T: Indexer,
{
    pub indexer: Arc<T>,
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, Ord, PartialOrd)]
#[serde(from = "String")]
pub struct Bytes32(String);

impl From<String> for Bytes32 {
    fn from(s: String) -> Self {
        Self(s.trim_start_matches("0x").into())
    }
}

impl fmt::Display for Bytes32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<String> for Bytes32 {
    fn into(self: Bytes32) -> String {
        format!("{}", self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ProofOfIndexing<T>
where
    T: Indexer,
{
    pub indexer: Arc<T>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct POICrossCheckReport<T>
where
    T: Indexer,
{
    pub poi1: ProofOfIndexing<T>,
    pub poi2: ProofOfIndexing<T>,
    pub diverging_block: Option</* TODO */ ()>,
}

#[derive(Debug)]
pub struct POIRequest {
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
}
