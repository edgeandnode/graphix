use serde::Serialize;
use std::{fmt, ops::Deref, sync::Arc};

use crate::{db::models::WritablePoI, indexer::Indexer};

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

#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct IndexingStatus<I>
where
    I: Indexer,
{
    pub indexer: Arc<I>,
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
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

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ProofOfIndexing<I>
where
    I: Indexer,
{
    pub indexer: Arc<I>,
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
    pub proof_of_indexing: Bytes32,
}

impl<I: Indexer> WritablePoI for ProofOfIndexing<I> {
    fn deployment_cid(&self) -> &str {
        self.deployment.as_str()
    }

    fn indexer_id(&self) -> &str {
        self.indexer.id()
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

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
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
