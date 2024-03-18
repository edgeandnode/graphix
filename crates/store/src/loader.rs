use std::collections::HashMap;
use std::marker::PhantomData;

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::models::{self, BigIntId, IntId};
use crate::{schema, Store};

pub struct StoreLoader<T> {
    store: Store,
    phantom: PhantomData<T>,
}

impl<T> StoreLoader<T> {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            phantom: PhantomData,
        }
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<BigIntId> for StoreLoader<models::Block> {
    type Value = models::Block;
    type Error = String;

    async fn load(&self, keys: &[BigIntId]) -> Result<HashMap<BigIntId, Self::Value>, Self::Error> {
        use schema::blocks;

        Ok(blocks::table
            .filter(blocks::id.eq_any(keys))
            .load::<models::Block>(&mut self.store.conn_err_string().await?)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|block| (block.id, block))
            .collect())
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<IntId> for StoreLoader<models::SgDeployment> {
    type Value = models::SgDeployment;
    type Error = String;

    async fn load(&self, keys: &[IntId]) -> Result<HashMap<IntId, Self::Value>, Self::Error> {
        use schema::{sg_deployments as sgd, sg_names};

        Ok(sgd::table
            .left_join(sg_names::table)
            .select((
                sgd::id,
                sgd::ipfs_cid,
                sg_names::name.nullable(),
                sgd::network,
                sgd::created_at,
            ))
            .filter(sgd::id.eq_any(keys))
            .load::<models::SgDeployment>(&mut self.store.conn_err_string().await?)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|deployment| (deployment.id, deployment))
            .collect())
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<IntId> for StoreLoader<models::Network> {
    type Value = models::Network;
    type Error = String;

    async fn load(&self, keys: &[IntId]) -> Result<HashMap<IntId, Self::Value>, Self::Error> {
        use schema::networks;

        Ok(networks::table
            .filter(networks::id.eq_any(keys))
            .select((
                networks::id,
                (networks::id, networks::name, networks::caip2),
            ))
            .load::<(IntId, models::Network)>(&mut self.store.conn_err_string().await?)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(id, network)| (id, network))
            .collect())
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<IntId> for StoreLoader<models::Indexer> {
    type Value = models::Indexer;
    type Error = String;

    async fn load(&self, keys: &[IntId]) -> Result<HashMap<IntId, Self::Value>, Self::Error> {
        use schema::indexers;

        Ok(indexers::table
            .filter(indexers::id.eq_any(keys))
            .load::<models::Indexer>(&mut self.store.conn_err_string().await?)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|indexer| (indexer.id, indexer))
            .collect())
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<IntId> for StoreLoader<models::GraphNodeCollectedVersion> {
    type Value = models::GraphNodeCollectedVersion;
    type Error = String;

    async fn load(&self, keys: &[IntId]) -> Result<HashMap<IntId, Self::Value>, Self::Error> {
        use schema::graph_node_collected_versions;

        Ok(graph_node_collected_versions::table
            .filter(graph_node_collected_versions::id.eq_any(keys))
            .load::<models::GraphNodeCollectedVersion>(&mut self.store.conn_err_string().await?)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|version| (version.id, version))
            .collect())
    }
}

#[async_trait]
impl async_graphql::dataloader::Loader<IntId>
    for StoreLoader<models::IndexerNetworkSubgraphMetadata>
{
    type Value = models::IndexerNetworkSubgraphMetadata;
    type Error = String;

    async fn load(&self, keys: &[IntId]) -> Result<HashMap<IntId, Self::Value>, Self::Error> {
        use schema::indexer_network_subgraph_metadata;

        Ok(indexer_network_subgraph_metadata::table
            .filter(indexer_network_subgraph_metadata::id.eq_any(keys))
            .load::<models::IndexerNetworkSubgraphMetadata>(
                &mut self.store.conn_err_string().await?,
            )
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|metadata| (metadata.id, metadata))
            .collect())
    }
}
