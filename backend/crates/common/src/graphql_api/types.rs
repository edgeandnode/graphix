//! GraphQL API types.
//!
//! A few of these are shared with database models as well. Should we keep them
//! separate? It would be cleaner, but at the cost of some code duplication.

use async_graphql::*;
use diesel::deserialize::FromSqlRow;
use serde::{Deserialize, Serialize};

use crate::store::models::{self};

type HexBytesWith0xPrefix = String;
type UuidString = String;

pub use divergence_investigation::*;
pub use filters::*;

mod divergence_investigation {
    use super::*;

    #[derive(Debug, Copy, Clone, Enum, PartialEq, Eq, Serialize, Deserialize)]
    pub enum DivergenceInvestigationStatus {
        Pending,
        InProgress,
        Complete,
    }

    #[derive(Debug, Serialize, SimpleObject, Deserialize)]
    pub struct DivergenceInvestigationReport {
        pub uuid: UuidString,
        pub status: DivergenceInvestigationStatus,
        pub bisection_runs: Vec<BisectionRun>,
    }

    #[derive(Debug, Serialize, SimpleObject, Deserialize)]
    pub struct DivergenceBlockBounds {
        pub lower_bound: PartialBlock,
        pub upper_bound: PartialBlock,
    }

    #[derive(Debug, SimpleObject, Serialize, Deserialize)]
    pub struct GraphNodeBlockMetadata {
        pub block: PartialBlock,
        pub block_cache_contents: Option<serde_json::Value>,
        pub eth_call_cache_contents: Option<serde_json::Value>,
        pub entity_changes: Option<serde_json::Value>,
    }

    #[derive(Debug, SimpleObject, Serialize, Deserialize)]
    pub struct BisectionRun {
        pub uuid: UuidString,
        pub poi1: HexBytesWith0xPrefix,
        pub poi2: HexBytesWith0xPrefix,
        pub divergence_block_bounds: DivergenceBlockBounds,
    }

    #[derive(InputObject, Deserialize, Debug, Clone, FromSqlRow, Serialize)]
    pub struct DivergenceInvestigationRequest {
        pub pois: Vec<String>,
        pub query_block_caches: Option<bool>,
        pub query_eth_call_caches: Option<bool>,
        pub query_entity_changes: Option<bool>,
    }

    impl DivergenceInvestigationRequest {
        pub fn query_block_caches(&self) -> bool {
            self.query_block_caches.unwrap_or(true)
        }

        pub fn query_eth_call_caches(&self) -> bool {
            self.query_eth_call_caches.unwrap_or(true)
        }

        pub fn query_entity_changes(&self) -> bool {
            self.query_entity_changes.unwrap_or(true)
        }
    }

    #[derive(Debug, Clone, FromSqlRow)]
    pub struct DivergenceInvestigationRequestWithUuid {
        pub id: String,
        pub req: DivergenceInvestigationRequest,
    }
}

mod filters {
    use super::*;

    #[derive(Default, InputObject)]
    pub struct SgDeploymentsQuery {
        pub network: Option<String>,
        pub name: Option<String>,
        pub ipfs_cid: Option<String>,
        pub limit: Option<u32>,
    }

    #[derive(Default, InputObject)]
    pub struct PoisQuery {
        pub network: Option<String>,
        pub deployments: Vec<String>,
        pub block_range: Option<BlockRangeInput>,
        pub limit: Option<u16>,
    }

    #[derive(Default, InputObject)]
    pub struct IndexersQuery {
        pub address: Option<HexBytesWith0xPrefix>,
        pub limit: Option<u16>,
    }
}

#[derive(InputObject)]
pub struct BlockRangeInput {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

#[derive(SimpleObject, Debug)]
pub struct Network {
    pub name: String,
    pub caip2: Option<String>,
}

#[derive(SimpleObject, Debug)]
pub struct Block {
    pub network: Network,
    pub number: u64,
    pub hash: HexBytesWith0xPrefix,
}

/// A block number that may or may not also have an associated hash.
#[derive(Debug, Serialize, SimpleObject, Deserialize)]
pub struct PartialBlock {
    pub number: i64,
    pub hash: Option<String>,
}

#[derive(SimpleObject, Debug)]
pub struct Deployment {
    pub id: String,
}

#[derive(SimpleObject, Debug)]
pub struct ProofOfIndexing {
    pub block: Block,
    pub hash: String,
    pub deployment: Deployment,
    pub allocated_tokens: Option<u64>,
    pub indexer: Indexer,
}

#[derive(SimpleObject, Debug)]
pub struct Indexer {
    pub id: HexBytesWith0xPrefix,
    pub allocated_tokens: Option<u64>,
}

impl From<models::Indexer> for Indexer {
    fn from(indexer: models::Indexer) -> Self {
        Self {
            id: indexer.name.unwrap_or_default(),
            allocated_tokens: None, // TODO: we don't store this in the db yet
        }
    }
}

impl From<models::Poi> for ProofOfIndexing {
    fn from(poi: models::Poi) -> Self {
        Self {
            allocated_tokens: None,
            deployment: Deployment {
                id: poi.sg_deployment.cid.clone(),
            },
            hash: poi.poi_hex(),
            block: Block {
                network: Network {
                    name: "mainnet".to_string(),
                    caip2: None,
                },
                number: poi.block.number as u64,
                hash: hex::encode(poi.block.hash),
            },
            indexer: Indexer::from(models::Indexer::from(poi.indexer)),
        }
    }
}

#[derive(InputObject)]
#[graphql(input_name = "POICrossCheckReportRequest")]
struct POICrossCheckReportRequest {
    deployments: Vec<String>,
    indexer1: Option<String>,
    indexer2: Option<String>,
}

#[derive(SimpleObject)]
struct DivergingBlock {
    block: PartialBlock,
    proof_of_indexing1: String,
    proof_of_indexing2: String,
}

#[derive(SimpleObject)]
#[graphql(name = "POICrossCheckReport")]
pub struct PoiCrossCheckReport {
    timestamp: String,
    indexer1: String,
    indexer2: String,
    deployment: String,
    block: PartialBlock,
    proof_of_indexing1: String,
    proof_of_indexing2: String,
    diverging_block: Option<DivergingBlock>,
}

/// A specific indexer can use `PoiAgreementRatio` to check in how much agreement it is with other
/// indexers, given its own poi for each deployment. A consensus currently means a majority of
/// indexers agreeing on a particular POI.
#[derive(SimpleObject)]
#[graphql(name = "PoiAgreementRatio")]
pub struct PoiAgreementRatio {
    pub poi: String,
    pub deployment: Deployment,
    pub block: PartialBlock,

    /// Total number of indexers that have live pois for the deployment.
    pub total_indexers: i32,

    /// Number of indexers that agree on the POI with the specified indexer,
    /// including the indexer itself.
    pub n_agreeing_indexers: i32,

    /// Number of indexers that disagree on the POI with the specified indexer.
    pub n_disagreeing_indexers: i32,

    /// Indicates if a consensus on the POI exists among indexers.
    pub has_consensus: bool,

    /// Indicates if the specified indexer's POI is part of the consensus.
    pub in_consensus: bool,
}
