use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use async_graphql::SimpleObject;
use serde::Serialize;

use crate::indexer::Indexer;
use crate::store::models::WritablePoi;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd, SimpleObject)]
pub struct IndexerVersion {
    pub version: String,
    pub commit: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd)]
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
pub struct SubgraphDeployment(pub String);

impl Deref for SubgraphDeployment {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Eq)]
pub struct IndexingStatus {
    pub indexer: Arc<dyn Indexer>,
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
    pub earliest_block_num: u64,
}

impl PartialEq for IndexingStatus {
    fn eq(&self, other: &Self) -> bool {
        &*self.indexer == &*other.indexer
            && self.deployment == other.deployment
            && self.network == other.network
            && self.latest_block == other.latest_block
    }
}

/// A 32-byte array that can be easily converted to and from hex strings.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd)]
pub struct Bytes32(pub [u8; 32]);

impl TryFrom<&str> for Bytes32 {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self(hex::FromHex::from_hex(s.trim_start_matches("0x"))?))
    }
}

impl TryFrom<Vec<u8>> for Bytes32 {
    type Error = anyhow::Error;

    fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
        if v.len() != 32 {
            return Err(anyhow::anyhow!("Expected 32 bytes, got {}", v.len()));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&v);
        Ok(Self(bytes))
    }
}

impl fmt::Display for Bytes32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct ProofOfIndexing {
    pub indexer: Arc<dyn Indexer>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}

impl PartialEq for ProofOfIndexing {
    fn eq(&self, other: &Self) -> bool {
        &*self.indexer == &*other.indexer
            && self.deployment == other.deployment
            && self.block == other.block
            && self.proof_of_indexing == other.proof_of_indexing
    }
}

impl WritablePoi for ProofOfIndexing {
    fn deployment_cid(&self) -> &str {
        self.deployment.as_str()
    }

    fn indexer_name(&self) -> Option<Cow<String>> {
        self.indexer.name()
    }

    fn indexer_address(&self) -> Option<&[u8]> {
        self.indexer.address().map(AsRef::as_ref)
    }

    fn block(&self) -> BlockPointer {
        self.block.clone()
    }

    fn proof_of_indexing(&self) -> &[u8] {
        &self.proof_of_indexing.0
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct DivergingBlock {
    pub block: BlockPointer,
    pub proof_of_indexing1: Bytes32,
    pub proof_of_indexing2: Bytes32,
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
