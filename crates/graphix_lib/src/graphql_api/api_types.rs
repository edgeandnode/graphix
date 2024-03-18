use async_graphql::{ComplexObject, Context, Object, SimpleObject};
use common::{IndexerAddress, PoiBytes};
use graphix_common_types as common;
use graphix_store::models::{self, IntId};
use num_traits::cast::ToPrimitive;

use super::{ctx_data, ApiSchemaContext};

#[derive(Clone, derive_more::From)]
pub struct SubgraphDeployment {
    model: models::SgDeployment,
}

impl SubgraphDeployment {
    pub fn cid(&self) -> &str {
        &self.model.cid
    }

    pub fn name(&self) -> Option<&str> {
        self.model.name.as_deref()
    }
}

#[Object]
impl SubgraphDeployment {
    /// IPFS CID of the subgraph deployment.
    #[graphql(name = "cid")]
    async fn graphql_cid(&self) -> String {
        self.model.cid.clone()
    }

    /// Human-readable name of the subgraph deployment, if present.
    #[graphql(name = "name")]
    async fn graphql_name(&self) -> Option<String> {
        self.model.name.clone()
    }

    /// Network of the subgraph deployment.
    async fn network(&self, ctx: &Context<'_>) -> Result<Network, String> {
        let loader = &ctx_data(ctx).loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(Into::into)
            .and_then(|opt: Option<Network>| opt.ok_or_else(|| "Network not found".to_string()))
    }
}

/// A network where subgraph deployments are indexed.
#[derive(derive_more::From)]
pub struct Network {
    model: models::Network,
}

#[Object]
impl Network {
    /// Human-readable name of the network, following The Graph naming
    /// standards.
    pub async fn name(&self) -> &str {
        self.model.name.as_str()
    }

    /// CAIP-2 chain ID of the network, if it exists.
    pub async fn caip2(&self) -> Option<&str> {
        self.model.caip2.as_deref()
    }
}

/// An indexer that is known to Graphix.
#[derive(derive_more::From)]
pub struct Indexer {
    model: models::Indexer,
}

impl Indexer {
    pub fn address(&self) -> IndexerAddress {
        self.model.address
    }

    pub fn name(&self) -> Option<&str> {
        self.model.name.as_deref()
    }

    pub async fn graph_node_version(
        &self,
        ctx: &ApiSchemaContext,
    ) -> Result<Option<models::GraphNodeCollectedVersion>, String> {
        let loader = &ctx.loader_graph_node_collected_version;

        if let Some(id) = self.model.graph_node_version {
            loader.load_one(id).await.map_err(Into::into)
        } else {
            Ok(None)
        }
    }
}

#[Object]
impl Indexer {
    #[graphql(name = "address")]
    async fn graphql_address(&self) -> String {
        self.model.address.to_string()
    }

    async fn default_display_name(&self) -> Option<String> {
        self.model.name.clone()
    }

    /// The version of the indexer.
    #[graphql(name = "graphNodeVersion")]
    async fn graphql_graph_node_version(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<models::GraphNodeCollectedVersion>, String> {
        self.graph_node_version(ctx_data(ctx)).await
    }

    /// The network subgraph metadata of the indexer.
    async fn network_subgraph_metadata(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<IndexerNetworkSubgraphMetadata>, String> {
        let loader = &ctx_data(ctx).loader_indexer_network_subgraph_metadata;

        if let Some(id) = self.model.network_subgraph_metadata {
            loader
                .load_one(id)
                .await
                .map_err(Into::into)
                .map(|opt| opt.map(Into::into))
        } else {
            Ok(None)
        }
    }
}

#[derive(derive_more::From)]
pub struct IndexerNetworkSubgraphMetadata {
    model: models::IndexerNetworkSubgraphMetadata,
}

#[Object]
impl IndexerNetworkSubgraphMetadata {
    async fn geohash(&self) -> Option<String> {
        self.model.geohash.clone()
    }

    async fn indexer_url(&self) -> Option<String> {
        self.model.indexer_url.clone()
    }

    async fn staked_tokens(&self) -> f64 {
        self.model.staked_tokens.to_f64().unwrap()
    }

    async fn allocated_tokens(&self) -> f64 {
        self.model.allocated_tokens.to_f64().unwrap()
    }

    async fn locked_tokens(&self) -> f64 {
        self.model.locked_tokens.to_f64().unwrap()
    }

    async fn query_fees_collected(&self) -> f64 {
        self.model.query_fees_collected.to_f64().unwrap()
    }

    async fn query_fee_rebates(&self) -> f64 {
        self.model.query_fee_rebates.to_f64().unwrap()
    }

    async fn rewards_earned(&self) -> f64 {
        self.model.rewards_earned.to_f64().unwrap()
    }

    async fn indexer_indexing_rewards(&self) -> f64 {
        self.model.indexer_indexing_rewards.to_f64().unwrap()
    }

    async fn delegator_indexing_rewards(&self) -> f64 {
        self.model.delegator_indexing_rewards.to_f64().unwrap()
    }

    async fn last_updated_at(&self) -> chrono::NaiveDateTime {
        self.model.last_updated_at
    }
}

/// A block pointer for a specific network.
#[derive(derive_more::From)]
pub struct Block {
    model: models::Block,
}

impl Block {
    pub fn number(&self) -> u64 {
        self.model.number.try_into().unwrap()
    }

    pub fn number_i64(&self) -> i64 {
        self.model.number
    }

    pub fn hash(&self) -> common::BlockHash {
        self.model.hash.clone().into()
    }
}

#[Object]
impl Block {
    /// The block number (a.k.a. height).
    #[graphql(name = "number")]
    async fn graphql_number(&self) -> u64 {
        self.model.number.try_into().unwrap()
    }

    /// The block hash, expressed as a hex string with a '0x' prefix.
    #[graphql(name = "hash")]
    async fn graphql_hash(&self) -> common::BlockHash {
        self.model.hash.clone().into()
    }

    /// The network that this block belongs to.
    pub async fn network(&self, ctx: &Context<'_>) -> Result<Network, String> {
        let loader = &ctx_data(ctx).loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Network not found".to_string()))
    }
}

/// A PoI (proof of indexing) that was queried and collected by Graphix.
#[derive(derive_more::From)]
pub struct ProofOfIndexing {
    pub model: models::Poi,
}

impl ProofOfIndexing {
    /// The PoI's hash.
    pub fn hash(&self) -> common::PoiBytes {
        self.model.poi.clone().into()
    }

    /// The subgraph deployment that this PoI is for.
    pub async fn deployment(&self, ctx: &ApiSchemaContext) -> Result<SubgraphDeployment, String> {
        let loader = &ctx.loader_subgraph_deployment;

        loader
            .load_one(self.model.sg_deployment_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Subgraph deployment not found".to_string()))
            .map(Into::into)
    }

    /// The block height and hash for which this PoI is valid.
    pub async fn block(&self, ctx: &ApiSchemaContext) -> Result<Block, String> {
        let loader = &ctx.loader_block;

        loader
            .load_one(self.model.block_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Block not found".to_string()))
            .map(Into::into)
    }

    /// The indexer that produced this PoI.
    pub async fn indexer(&self, ctx: &ApiSchemaContext) -> Result<Indexer, String> {
        let loader = &ctx.loader_indexer;

        loader
            .load_one(self.model.indexer_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Indexer not found".to_string()))
            .map(Into::into)
    }
}

#[Object]
impl ProofOfIndexing {
    #[graphql(name = "block")]
    pub async fn graphql_block(&self, ctx: &Context<'_>) -> Result<Block, String> {
        self.block(ctx_data(ctx)).await
    }

    #[graphql(name = "hash")]
    async fn graphql_hash(&self) -> common::PoiBytes {
        self.hash()
    }

    #[graphql(name = "deployment")]
    pub async fn graphql_deployment(
        &self,
        ctx: &Context<'_>,
    ) -> Result<SubgraphDeployment, String> {
        self.deployment(ctx_data(ctx)).await
    }

    #[graphql(name = "indexer")]
    pub async fn graphql_indexer(&self, ctx: &Context<'_>) -> Result<Indexer, String> {
        self.indexer(ctx_data(ctx)).await
    }
}

/// A specific indexer can use `PoiAgreementRatio` to check in how much agreement it is with other
/// indexers, given its own poi for each deployment. A consensus currently means a majority of
/// indexers agreeing on a particular POI.
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct PoiAgreementRatio {
    pub poi: PoiBytes,
    #[graphql(skip)]
    pub deployment_id: IntId,
    pub block: Block,

    /// Total number of indexers that have live pois for the deployment.
    pub total_indexers: u32,

    /// Number of indexers that agree on the POI with the specified indexer,
    /// including the indexer itself.
    pub n_agreeing_indexers: u32,

    /// Number of indexers that disagree on the POI with the specified indexer.
    pub n_disagreeing_indexers: u32,

    /// Indicates if a consensus on the POI exists among indexers.
    pub has_consensus: bool,

    /// Indicates if the specified indexer's POI is part of the consensus.
    pub in_consensus: bool,
}

#[ComplexObject]
impl PoiAgreementRatio {
    async fn deployment(&self, ctx: &Context<'_>) -> Result<SubgraphDeployment, String> {
        let loader = &ctx_data(ctx).loader_subgraph_deployment;

        loader
            .load_one(self.deployment_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Subgraph deployment not found".to_string()))
            .map(Into::into)
    }
}
