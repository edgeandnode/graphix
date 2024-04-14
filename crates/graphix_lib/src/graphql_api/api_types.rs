use async_graphql::{ComplexObject, Context, Object, SimpleObject};
use common::{IndexerAddress, IpfsCid};
use graphix_common_types as common;
use graphix_store::models::{self, IntId};
use num_traits::cast::ToPrimitive;

use super::{ctx_data, ApiSchemaContext};

#[derive(Clone, derive_more::From)]
pub struct SubgraphDeployment {
    model: models::SgDeployment,
}

impl SubgraphDeployment {
    pub fn cid(&self) -> &IpfsCid {
        &self.model.cid
    }

    pub fn name(&self) -> Option<&str> {
        self.model.name.as_deref()
    }

    pub async fn network(&self, ctx: &ApiSchemaContext) -> Result<Network, String> {
        let loader = &ctx.loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(Into::into)
            .and_then(|opt: Option<Network>| opt.ok_or_else(|| "Network not found".to_string()))
    }
}

#[Object]
impl SubgraphDeployment {
    /// IPFS CID of the subgraph deployment.
    #[graphql(name = "cid")]
    async fn graphql_cid(&self) -> IpfsCid {
        self.model.cid.clone()
    }

    /// Human-readable name of the subgraph deployment, if present.
    #[graphql(name = "name")]
    async fn graphql_name(&self) -> Option<String> {
        self.model.name.clone()
    }

    /// Network of the subgraph deployment.
    #[graphql(name = "network")]
    async fn graphql_network(&self, ctx: &Context<'_>) -> Result<Network, String> {
        self.network(ctx_data(ctx)).await
    }
}

/// A network where subgraph deployments are indexed.
#[derive(derive_more::From)]
pub struct Network {
    model: models::Network,
}

impl Network {
    pub fn name(&self) -> &str {
        self.model.name.as_str()
    }

    pub fn caip2(&self) -> Option<&str> {
        self.model.caip2.as_deref()
    }
}

#[Object]
impl Network {
    /// Human-readable name of the network, following The Graph naming
    /// standards.
    #[graphql(name = "name")]
    pub async fn graphql_name(&self) -> &str {
        self.name()
    }

    /// CAIP-2 chain ID of the network, if it exists.
    #[graphql(name = "caip2")]
    pub async fn graphql_caip2(&self) -> Option<&str> {
        self.caip2()
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

    pub async fn network(&self, ctx: &ApiSchemaContext) -> Result<Network, String> {
        let loader = &ctx.loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| {
                opt.ok_or_else(|| "Network not found".to_string())
                    .map(Into::into)
            })
    }
}

#[Object]
impl Block {
    /// Returns an estimate of the timestamp of the block, based on the
    /// network's block speed and the block's number.
    #[graphql(name = "estimatedTimestamp")]
    pub async fn graphql_estimated_timestamp(
        &self,
        ctx: &Context<'_>,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let network = self.network(ctx_data(ctx)).await.ok()?;
        let chain_config = ctx_data(ctx).config.chains.get(network.name())?;
        let speed_config = chain_config.speed.as_ref()?;

        let duration_per_block =
            chrono::Duration::milliseconds(speed_config.avg_block_time_in_msecs.try_into().ok()?);

        Some(speed_config.sample_timestamp + duration_per_block * self.number().try_into().ok()?)
    }

    /// Returns an URL to a block explorer page for the block, if configured.
    #[graphql(name = "blockExplorerUrl")]
    pub async fn graphql_block_explorer_url(&self, ctx: &Context<'_>) -> Option<String> {
        let network = self.network(ctx_data(ctx)).await.ok()?;
        let chain_config = ctx_data(ctx).config.chains.get(network.name())?;

        let block_explorer_url_template = chain_config
            .block_explorer_url_template_for_block
            .as_ref()?;

        Some(block_explorer_url_template.url_for_block(self.number()))
    }

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
    #[graphql(name = "network")]
    pub async fn graphql_network(&self, ctx: &Context<'_>) -> Result<Network, String> {
        self.network(ctx_data(ctx)).await
    }
}

/// A PoI (proof of indexing) that was queried and collected by Graphix.
#[derive(derive_more::From)]
pub struct ProofOfIndexing {
    // FIXME: ideally shouldn't be public.
    pub model: models::Poi,
}

impl ProofOfIndexing {
    pub fn hash(&self) -> common::PoiBytes {
        self.model.poi.clone().into()
    }

    pub async fn deployment(&self, ctx: &ApiSchemaContext) -> Result<SubgraphDeployment, String> {
        let loader = &ctx.loader_subgraph_deployment;

        loader
            .load_one(self.model.sg_deployment_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Subgraph deployment not found".to_string()))
            .map(Into::into)
    }

    pub async fn block(&self, ctx: &ApiSchemaContext) -> Result<Block, String> {
        let loader = &ctx.loader_block;

        loader
            .load_one(self.model.block_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Block not found".to_string()))
            .map(Into::into)
    }

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
    /// The block height and hash for which this PoI is valid.
    #[graphql(name = "block")]
    async fn graphql_block(&self, ctx: &Context<'_>) -> Result<Block, String> {
        self.block(ctx_data(ctx)).await
    }

    /// The PoI's hash.
    #[graphql(name = "hash")]
    async fn graphql_hash(&self) -> common::PoiBytes {
        self.hash()
    }

    /// The subgraph deployment that this PoI is for.
    #[graphql(name = "deployment")]
    async fn graphql_deployment(&self, ctx: &Context<'_>) -> Result<SubgraphDeployment, String> {
        self.deployment(ctx_data(ctx)).await
    }

    /// The indexer that produced this PoI.
    #[graphql(name = "indexer")]
    async fn graphql_indexer(&self, ctx: &Context<'_>) -> Result<Indexer, String> {
        self.indexer(ctx_data(ctx)).await
    }
}

/// A specific indexer can use `PoiAgreementRatio` to check in how much agreement it is with other
/// indexers, given its own poi for each deployment. A consensus currently means a majority of
/// indexers agreeing on a particular POI.
#[derive(SimpleObject, Debug)]
#[graphql(complex)]
pub struct PoiAgreementRatio {
    #[graphql(skip)]
    pub poi_id: IntId,

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
    /// The PoI in question.
    #[graphql(name = "poi")]
    async fn graphql_poi(&self, ctx: &Context<'_>) -> Result<ProofOfIndexing, String> {
        let loader = &ctx_data(ctx).loader_poi;

        loader
            .load_one(self.poi_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "PoI not found".to_string()))
            .map(Into::into)
    }
}
