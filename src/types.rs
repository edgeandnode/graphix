use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct BlockPointer {
    pub number: u64,
    pub hash: String,
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
