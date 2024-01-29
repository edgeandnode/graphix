//! GraphQL API types.
//!
//! A few of these are shared with database models as well. Should we keep them
//! separate? It would be cleaner, but at the cost of some code duplication.

use async_graphql::*;
use diesel::deserialize::FromSqlRow;
use serde::{Deserialize, Serialize};

//use crate::indexer::IndexerId;
//use crate::types::IndexerVersion;

type HexBytesWith0xPrefix = String;
type UuidString = String;

pub use divergence_investigation::*;
pub use filters::*;

mod divergence_investigation {
    use super::*;

    /// Once Graphix launches a PoI divergence investigation, its status value
    /// can be one of these.
    #[derive(Debug, Copy, Clone, Enum, PartialEq, Eq, Serialize, Deserialize)]
    pub enum DivergenceInvestigationStatus {
        /// The investigation has been requested, but not yet launched and it's
        /// scheduled to be launched soon.
        Pending,
        /// The investigation has been launched, some requests have possibly
        /// been sent already, but the investigation is not concluded. Some
        /// information may be available already, but partial.
        InProgress,
        /// The investigation has been concluded and the end results are
        /// available.
        Complete,
    }

    /// A divergence investigation report contains all information that pertains to a divergence
    /// investigation, including the results of its bisection run(s).
    #[derive(Debug, Serialize, SimpleObject, Deserialize)]
    pub struct DivergenceInvestigationReport {
        /// The UUID of the divergence investigation request that this report
        /// pertains to. This UUID is also used to identify the report, as well
        /// as the request.
        pub uuid: UuidString,
        /// The latest known status of the divergence investigation.
        pub status: DivergenceInvestigationStatus,
        /// A list of bisection runs that were performed as part of this
        /// divergence investigation. If the investigation is still in progress,
        /// this list may be incomplete.
        pub bisection_runs: Vec<BisectionRunReport>,
        /// If the divergence investigation failed altogether, this field
        /// contains the error message. Please note that specific bisection runs
        /// may also fail, in which case the error message will be in the
        /// `error` field of the corresponding `BisectionRunReport`.
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, SimpleObject, Deserialize)]
    pub struct DivergenceBlockBounds {
        pub lower_bound: PartialBlock,
        pub upper_bound: PartialBlock,
    }

    /// When Graphix investigates a divergence between two indexers, it runs a
    /// bisection algorithm and collects useful information about each block
    /// from the indexer's `graph-node` instance through its public GraphQL API.
    /// This metadata is then available in divergence investigation reports.
    #[derive(Debug, SimpleObject, Serialize, Deserialize)]
    pub struct GraphNodeBlockMetadata {
        /// The block number and hash that this metadata pertains to.
        pub block: PartialBlock,
        /// The contents of `graph-node`'s block cache for this block, if
        /// requested and available.
        pub block_cache_contents: Option<serde_json::Value>,
        /// The contents of `graph-node`'s eth call cache for this block, if
        /// requested and available.
        pub eth_call_cache_contents: Option<serde_json::Value>,
        /// A list of entitity changes produced by `graph-node` for this block
        /// and subgraph deployment,
        /// if requested and available.
        pub entity_changes: Option<serde_json::Value>,
    }

    /// A bisection run report contains information about a specific bisection
    /// run that is part of a larger divergence investigation.
    #[derive(Debug, Clone, SimpleObject, Serialize, Deserialize)]
    pub struct BisectionRunReport {
        /// The UUID of the bisection run that this report pertains to. This UUID
        /// is different from the UUID of the parent divergence investigation
        /// request.
        pub uuid: UuidString,
        /// The first PoI that was used to start the bisection run.
        pub poi1: HexBytesWith0xPrefix,
        /// The second PoI that was used to start the bisection run.
        pub poi2: HexBytesWith0xPrefix,
        /// The lower and upper block bounds inside which the bisection run
        /// occurred.
        pub divergence_block_bounds: DivergenceBlockBounds,
        /// For each specific bisection, a list of bisection reports is
        /// available which includes the block number and hash, as well as the
        /// metadata that was collected from `graph-node` for that block.
        pub bisects: Vec<BisectionReport>,
        /// If the bisection run failed before reaching a conclusion at a single
        /// block, this field contains the error message.
        pub error: Option<String>,
    }

    /// Metadata that was collected during a bisection run.
    #[derive(Debug, Clone, SimpleObject, Serialize, Deserialize)]
    pub struct BisectionReport {
        /// The block number and hash that this metadata pertains to.
        pub block: PartialBlock,
        /// The metadata that was collected from the first indexer's
        /// `graph-node` instance.
        pub indexer1_response: String,
        /// The metadata that was collected from the second indexer's
        /// `graph-node` instance.
        pub indexer2_response: String,
    }

    /// The type of a new divergence investigation request that the API user
    /// can submit.
    #[derive(InputObject, Deserialize, Debug, Clone, FromSqlRow, Serialize)]
    pub struct DivergenceInvestigationRequest {
        /// A list of PoI hashes that should be investigated for divergence.
        /// If this list contains more than two PoIs, a new bisection run will be performed
        /// for each unordered pair of PoIs.
        pub pois: Vec<String>,
        /// Indicates whether to collect `graph-node`'s block cache contents
        /// during bisection runs to include in the report.
        pub query_block_caches: Option<bool>,
        /// Indicates whether to collect `graph-node`'s eth call cache contents
        /// during bisection runs to include in the report.
        pub query_eth_call_caches: Option<bool>,
        /// Indicates whether to collect `graph-node`'s entity changes during
        /// bisection runs to include in the report.
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

    /// A filter for subgraph deployments.
    #[derive(Default, InputObject)]
    pub struct SgDeploymentsQuery {
        /// What network the subgraph indexes.
        pub network: Option<String>,
        /// The human-readable name of the queried subgraph deployment(s).
        pub name: Option<String>,
        /// The IPFS hash of the subgraph deployment(s).
        pub ipfs_cid: Option<String>,
        /// Upper limit on the number of shown results.
        pub limit: Option<u32>,
    }

    /// A filter for PoIs (proofs of indexing).
    #[derive(Default, InputObject)]
    pub struct PoisQuery {
        /// Restricts the query to PoIs for subgraph deployments that index the
        /// given chain name.
        pub network: Option<String>,
        /// Restricts the query to PoIs for these given subgraph deployments (by
        /// hex-encoded IPFS CID with '0x' prefix).
        pub deployments: Vec<String>,
        /// Restricts the query to PoIs that were collected in the given block
        /// range.
        pub block_range: Option<BlockRangeInput>,
        /// Upper limit on the number of shown results.
        pub limit: Option<u16>,
    }

    /// A filter for indexers.
    #[derive(Default, InputObject)]
    pub struct IndexersQuery {
        /// The address of the indexer, encoded as a hex string with a '0x'
        /// prefix.
        pub address: Option<HexBytesWith0xPrefix>,
        /// Upper limit on the number of shown results.
        pub limit: Option<u16>,
    }
}

/// A block range, specified by optional start and end block numbers.
#[derive(InputObject)]
pub struct BlockRangeInput {
    /// The start block number (inclusive).
    pub start: Option<u64>,
    /// The end block number (inclusive).
    pub end: Option<u64>,
}

/// A network where subgraph deployments are indexed.
#[derive(SimpleObject, Debug)]
pub struct Network {
    /// Human-readable name of the network, following The Graph naming
    /// conventions.
    pub name: String,
    /// CAIP-2 chain ID of the network, if it exists.
    pub caip2: Option<String>,
}

/// A block pointer for a specific network.
#[derive(SimpleObject, Debug)]
pub struct Block {
    /// The network that this block belongs to.
    pub network: Network,
    /// The block number (or height).
    pub number: u64,
    /// The block hash, expressed as a hex string with a '0x' prefix.
    pub hash: HexBytesWith0xPrefix,
}

/// A block number that may or may not also have an associated hash.
#[derive(Debug, Clone, Serialize, SimpleObject, Deserialize)]
pub struct PartialBlock {
    /// The block number (or height).
    pub number: i64,
    /// The block hash, if known. Expressed as a hex string with a '0x' prefix.
    pub hash: Option<String>,
}

#[derive(SimpleObject, Debug)]
pub struct Deployment {
    pub id: String,
}

/// A PoI (proof of indexing) that was queried and collected by Graphix.
#[derive(SimpleObject, Debug)]
pub struct ProofOfIndexing {
    /// The block height and hash for which this PoI is valid.
    pub block: Block,
    /// The PoI's hash.
    pub hash: String,
    /// The subgraph deployment that this PoI is for.
    pub deployment: Deployment,
    /// The amount of allocated tokens by the indexer for this PoI, if known.
    pub allocated_tokens: Option<u64>,
    /// The indexer that produced this PoI.
    pub indexer: Indexer,
}

/// An indexer that is known to Graphix.
#[derive(SimpleObject, Debug)]
pub struct Indexer {
    pub id: String,
    pub name: Option<String>,
    pub address: Option<HexBytesWith0xPrefix>,
    pub version: Option<IndexerVersion>,
    /// The number of tokens allocated to the indexer, if known.
    pub allocated_tokens: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd, SimpleObject)]
pub struct IndexerVersion {
    pub version: String,
    pub commit: String,
}

#[derive(InputObject)]
#[graphql(input_name = "POICrossCheckReportRequest")]
struct POICrossCheckReportRequest {
    deployments: Vec<String>,
    indexer1: Option<String>,
    indexer2: Option<String>,
}

#[derive(SimpleObject)]
pub struct DivergingBlock {
    pub block: PartialBlock,
    pub proof_of_indexing1: String,
    pub proof_of_indexing2: String,
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
