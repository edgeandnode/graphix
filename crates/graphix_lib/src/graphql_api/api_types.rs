use async_graphql::{ComplexObject, Context, Object, SimpleObject};
use common::{PartialBlock, PoiBytes};
use graphix_common_types as common;
use graphix_store::models::{self, IntId};
use num_traits::cast::ToPrimitive;

use super::ctx_data;

#[derive(derive_more::From)]
pub struct SubgraphDeployment {
    model: models::SgDeployment,
}

#[Object]
impl SubgraphDeployment {
    /// IPFS CID of the subgraph deployment.
    pub async fn cid(&self) -> String {
        self.model.cid.clone()
    }

    /// Human-readable name of the subgraph deployment, if present.
    async fn name(&self) -> Option<String> {
        self.model.name.clone()
    }

    /// Network of the subgraph deployment.
    async fn network(&self, ctx: &Context<'_>) -> Result<common::Network, String> {
        let loader = &ctx_data(ctx).loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Network not found".to_string()))
    }
}

/// An indexer that is known to Graphix.
#[derive(derive_more::From)]
pub struct Indexer {
    model: models::Indexer,
}

#[Object]
impl Indexer {
    async fn address(&self) -> String {
        self.model.address.to_string()
    }

    async fn default_display_name(&self) -> Option<String> {
        self.model.name.clone()
    }

    /// The version of the indexer.
    async fn graph_node_version(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<models::GraphNodeCollectedVersion>, String> {
        let loader = &ctx_data(ctx).loader_graph_node_collected_version;

        if let Some(id) = self.model.graph_node_version {
            loader.load_one(id).await.map_err(Into::into)
        } else {
            Ok(None)
        }
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

#[derive(derive_more::From)]
pub struct Block {
    model: models::Block,
}

#[Object]
impl Block {
    async fn number(&self) -> u64 {
        self.model.number.try_into().unwrap()
    }

    async fn hash(&self) -> common::BlockHash {
        self.model.hash.clone().into()
    }

    async fn network(&self, ctx: &Context<'_>) -> Result<common::Network, String> {
        let loader = &ctx_data(ctx).loader_network;

        loader
            .load_one(self.model.network_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Network not found".to_string()))
    }
}

#[derive(derive_more::From)]
pub struct ProofOfIndexing {
    pub model: models::Poi,
}

#[Object]
impl ProofOfIndexing {
    pub async fn block(&self, ctx: &Context<'_>) -> Result<Block, String> {
        let loader = &ctx_data(ctx).loader_block;

        loader
            .load_one(self.model.block_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Block not found".to_string()))
            .map(Into::into)
    }

    pub async fn hash(&self) -> common::PoiBytes {
        self.model.poi.clone().into()
    }

    pub async fn deployment(&self, ctx: &Context<'_>) -> Result<SubgraphDeployment, String> {
        let loader = &ctx_data(ctx).loader_subgraph_deployment;

        loader
            .load_one(self.model.sg_deployment_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Subgraph deployment not found".to_string()))
            .map(Into::into)
    }

    async fn indexer(&self, ctx: &Context<'_>) -> Result<Indexer, String> {
        let loader = &ctx_data(ctx).loader_indexer;

        loader
            .load_one(self.model.indexer_id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| "Indexer not found".to_string()))
            .map(Into::into)
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
