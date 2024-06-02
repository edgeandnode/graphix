//! GraphQL API types.
//!
//! A few of these are shared with database models as well. Should we keep them
//! separate? It would be cleaner, but at the cost of some code duplication.

mod hex_string;
pub mod inputs;
mod ipfs_cid;

use async_graphql::*;
use chrono::NaiveDateTime;
pub use divergence_investigation::*;
pub use hex_string::HexString;
pub use ipfs_cid::IpfsCid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A PoI (proof of indexing) is always 32 bytes.
pub type PoiBytes = HexString<[u8; 32]>;

/// Note that block hashes have variable length, to easily deal with different
/// hash sizes across networks.
pub type BlockHash = HexString<Vec<u8>>;

/// Ethereum addresses, and indexers' as a consequence, are 20 bytes long.
pub type IndexerAddress = HexString<[u8; 20]>;

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
        pub uuid: Uuid,
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
        pub uuid: Uuid,
        /// The first PoI that was used to start the bisection run.
        pub poi1: PoiBytes,
        /// The second PoI that was used to start the bisection run.
        pub poi2: PoiBytes,
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
}

/// A block number that may or may not also have an associated hash.
#[derive(Debug, Clone, Serialize, SimpleObject, Deserialize)]
pub struct PartialBlock {
    /// The block number (or height).
    pub number: i64,
    /// The block hash, if known. Expressed as a hex string with a '0x' prefix.
    pub hash: Option<BlockHash>,
}

#[derive(SimpleObject, Debug)]
pub struct Deployment {
    pub id: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Ord, PartialOrd, SimpleObject)]
pub struct GraphNodeCollectedVersion {
    pub version: Option<String>,
    pub commit: Option<String>,
    pub error_response: Option<String>,
    pub collected_at: NaiveDateTime,
}

#[derive(SimpleObject)]
pub struct DivergingBlock {
    pub block: PartialBlock,
    pub proof_of_indexing1: PoiBytes,
    pub proof_of_indexing2: PoiBytes,
}

#[derive(SimpleObject)]
#[graphql(name = "POICrossCheckReport")]
pub struct PoiCrossCheckReport {
    timestamp: String,
    indexer1: String,
    indexer2: String,
    deployment: String,
    block: PartialBlock,
    proof_of_indexing1: PoiBytes,
    proof_of_indexing2: PoiBytes,
    diverging_block: Option<DivergingBlock>,
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    async_graphql::Enum,
    // strum is used for (de)serialization in the database.
    strum::Display,
    strum::EnumString,
)]
pub enum ApiKeyPermissionLevel {
    Admin,
}
