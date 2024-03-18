pub mod api_types;
mod server;

use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, EmptySubscription, Schema, SchemaBuilder};
use graphix_store::{Store, StoreLoader};

use self::server::{MutationRoot, QueryRoot};
use crate::config::Config;

pub type ApiSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct ApiSchemaContext {
    pub store: Store,
    pub loader_poi: DataLoader<StoreLoader<graphix_store::models::Poi>>,
    pub loader_network: DataLoader<StoreLoader<graphix_store::models::Network>>,
    pub loader_graph_node_collected_version:
        DataLoader<StoreLoader<graphix_store::models::GraphNodeCollectedVersion>>,
    pub loader_indexer_network_subgraph_metadata:
        DataLoader<StoreLoader<graphix_store::models::IndexerNetworkSubgraphMetadata>>,
    pub loader_block: DataLoader<StoreLoader<graphix_store::models::Block>>,
    pub loader_indexer: DataLoader<StoreLoader<graphix_store::models::Indexer>>,
    pub loader_subgraph_deployment: DataLoader<StoreLoader<graphix_store::models::SgDeployment>>,
    pub config: Config,
}

impl ApiSchemaContext {
    pub fn new(store: Store, config: Config) -> Self {
        let loader_poi = DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_network = DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_graph_node_collected_version =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_indexer_network_subgraph_metadata =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_block = DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_indexer = DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);
        let loader_subgraph_deployment =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn);

        Self {
            store,
            loader_poi,
            loader_network,
            loader_graph_node_collected_version,
            loader_indexer_network_subgraph_metadata,
            loader_block,
            loader_indexer,
            loader_subgraph_deployment,

            config,
        }
    }
}

pub fn api_schema_builder() -> SchemaBuilder<QueryRoot, MutationRoot, EmptySubscription> {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
}

pub fn api_schema(ctx: ApiSchemaContext) -> ApiSchema {
    api_schema_builder().data(ctx).finish()
}

pub fn ctx_data<'a>(ctx: &'a Context) -> &'a ApiSchemaContext {
    ctx.data::<ApiSchemaContext>()
        .expect("Failed to get API context")
}
