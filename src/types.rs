use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct BlockPointer {
    pub number: u64,
    pub hash: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SubgraphDeployment(pub String);

impl Deref for SubgraphDeployment {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexingStatus {
    pub deployment: SubgraphDeployment,
    pub network: String,
    pub latest_block: BlockPointer,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
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
