//! Structs and complex datatypes that may serve as inputs, filters, or requests
//! for the GraphQL API.

use std::ops::{Bound, RangeBounds};

use async_graphql::InputObject;

use crate::{IndexerAddress, IpfsCid};

/// A filter for subgraph deployments.
#[derive(Default)]
pub struct SgDeploymentsQuery {
    /// What network the subgraph indexes.
    pub network_name: Option<String>,
    /// The human-readable name of the queried subgraph deployment(s).
    pub name: Option<String>,
    /// The IPFS hash of the subgraph deployment(s).
    pub ipfs_cid: Option<IpfsCid>,
    /// Upper limit on the number of shown results.
    pub limit: Option<u16>,
}

/// A filter for PoIs (proofs of indexing).
#[derive(Default, InputObject)]
pub struct PoisQuery {
    /// Restricts the query to PoIs for subgraph deployments that index the
    /// given chain name.
    pub network: Option<String>,
    /// Restricts the query to PoIs for these given subgraph deployment IDs.
    pub deployments: Vec<IpfsCid>,
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
