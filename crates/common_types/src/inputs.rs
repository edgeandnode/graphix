//! Structs and complex datatypes that may serve as inputs, filters, or requests
//! for the GraphQL API.

use std::ops::{Bound, RangeBounds};

use async_graphql::InputObject;
use diesel::deserialize::FromSqlRow;
use serde::{Deserialize, Serialize};

use crate::{IndexerAddress, PoiBytes};

/// The type of a new divergence investigation request that the API user
/// can submit.
#[derive(InputObject, Deserialize, Debug, Clone, FromSqlRow, Serialize)]
pub struct DivergenceInvestigationRequest {
    /// A list of PoI hashes that should be investigated for divergence.
    /// If this list contains more than two PoIs, a new bisection run will be performed
    /// for each unordered pair of PoIs.
    #[graphql(validator(max_items = 4))]
    pub pois: Vec<PoiBytes>,
    /// Indicates whether to collect `graph-node`'s block cache contents
    /// during bisection runs to include in the report.
    #[graphql(default = true)]
    pub query_block_caches: bool,
    /// Indicates whether to collect `graph-node`'s eth call cache contents
    /// during bisection runs to include in the report.
    #[graphql(default = true)]
    pub query_eth_call_caches: bool,
    /// Indicates whether to collect `graph-node`'s entity changes during
    /// bisection runs to include in the report.
    #[graphql(default = true)]
    pub query_entity_changes: bool,
}

/// A filter for subgraph deployments.
#[derive(Default, InputObject)]
pub struct SgDeploymentsQuery {
    /// What network the subgraph indexes.
    pub network_name: Option<String>,
    /// The human-readable name of the queried subgraph deployment(s).
    pub name: Option<String>,
    /// The IPFS hash of the subgraph deployment(s).
    pub ipfs_cid: Option<String>,
    /// Upper limit on the number of shown results.
    pub limit: Option<u16>,
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
    pub block_range: Option<BlockRange>,
    /// Upper limit on the number of shown results.
    pub limit: Option<u16>,
}

/// A filter for indexers.
#[derive(Default, InputObject)]
pub struct IndexersQuery {
    /// The address of the indexer, encoded as a hex string with a '0x'
    /// prefix.
    pub address: Option<IndexerAddress>,
    /// Upper limit on the number of shown results.
    pub limit: Option<u16>,
}

/// A block range, specified by optional start and end block numbers.
#[derive(InputObject)]
pub struct BlockRange {
    /// The start block number (inclusive).
    pub start: Option<u64>,
    /// The end block number (inclusive).
    pub end: Option<u64>,
}

impl RangeBounds<u64> for BlockRange {
    fn start_bound(&self) -> Bound<&u64> {
        match self.start {
            Some(ref start) => Bound::Included(start),
            None => Bound::Unbounded,
        }
    }

    fn end_bound(&self) -> Bound<&u64> {
        match self.end {
            Some(ref end) => Bound::Included(end),
            None => Bound::Unbounded,
        }
    }
}
