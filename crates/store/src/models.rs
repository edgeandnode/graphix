use std::borrow::Cow;

use async_graphql::SimpleObject;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::sql_types::Jsonb;
use diesel::{AsChangeset, AsExpression, FromSqlRow, Insertable, Queryable, Selectable};
use graphix_common_types as types;
use graphix_indexer_client::IndexerId;
use serde::{Deserialize, Serialize};
use types::{BlockHash, Deployment, IndexerAddress, Network, PoiBytes};

use super::schema::*;

pub type IntId = i32;
pub type BigIntId = i64;
pub type SgDeploymentCid = String;

#[derive(Queryable, Serialize, Debug)]
pub struct FailedQueryRow {
    pub indexer_id: IntId,
    pub query_name: String,
    pub raw_query: String,
    pub response: String,
    pub timestamp: NaiveDateTime,
}

#[derive(Queryable, Serialize, Debug)]
pub struct Poi {
    pub id: IntId,
    pub poi: PoiBytes,
    pub sg_deployment_id: IntId,
    pub indexer_id: IntId,
    pub block_id: BigIntId,
    pub created_at: NaiveDateTime,
}

#[derive(Selectable, Insertable, Debug)]
#[diesel(table_name = graph_node_collected_versions)]
pub struct NewGraphNodeCollectedVersion {
    pub version_string: Option<String>,
    pub version_commit: Option<String>,
    pub error_response: Option<String>,
}

#[derive(Queryable, Clone, Selectable, Debug, SimpleObject)]
#[diesel(table_name = graph_node_collected_versions)]
pub struct GraphNodeCollectedVersion {
    #[graphql(skip)]
    pub id: IntId,
    pub version_string: Option<String>,
    pub version_commit: Option<String>,
    pub error_response: Option<String>,
    pub collected_at: NaiveDateTime,
}

impl GraphNodeCollectedVersion {
    pub fn into_common_type(self) -> types::GraphNodeCollectedVersion {
        types::GraphNodeCollectedVersion {
            version: self.version_string,
            commit: self.version_commit,
            error_response: self.error_response,
            collected_at: self.collected_at,
        }
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = pois)]
pub struct NewPoi {
    pub poi: PoiBytes,
    pub created_at: NaiveDateTime,
    pub sg_deployment_id: IntId,
    pub indexer_id: IntId,
    pub block_id: BigIntId,
}

#[derive(Queryable, Clone, Debug, Serialize)]
pub struct Block {
    pub id: BigIntId,
    pub network_id: IntId,
    pub number: i64,
    pub hash: BlockHash,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = blocks)]
pub struct NewBlock {
    pub network_id: IntId,
    pub number: i64,
    pub hash: BlockHash,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = indexers)]
pub struct Indexer {
    pub id: IntId,
    pub address: IndexerAddress,
    pub name: Option<String>,
    pub graph_node_version: Option<IntId>,
    pub network_subgraph_metadata: Option<IntId>,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
}

impl Indexer {
    pub fn into_common_type(self, version: Option<GraphNodeCollectedVersion>) -> types::Indexer {
        types::Indexer {
            address: self.address,
            default_display_name: self.name,
            graph_node_version: version.map(GraphNodeCollectedVersion::into_common_type),
            network_subgraph_metadata: None,
        }
    }
}

impl IndexerId for Indexer {
    fn address(&self) -> IndexerAddress {
        self.address
    }

    fn name(&self) -> Option<Cow<str>> {
        match &self.name {
            Some(name) => Some(Cow::Borrowed(name)),
            None => None,
        }
    }
}

#[derive(Debug, Insertable, AsChangeset, Serialize)]
#[diesel(table_name = indexer_network_subgraph_metadata)]
pub struct NewIndexerNetworkSubgraphMetadata {
    pub geohash: Option<String>,
    pub indexer_url: Option<String>,
    pub staked_tokens: BigDecimal,
    pub allocated_tokens: BigDecimal,
    pub locked_tokens: BigDecimal,
    pub query_fees_collected: BigDecimal,
    pub query_fee_rebates: BigDecimal,
    pub rewards_earned: BigDecimal,
    pub indexer_indexing_rewards: BigDecimal,
    pub delegator_indexing_rewards: BigDecimal,
    pub last_updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = indexer_network_subgraph_metadata)]
pub struct IndexerNetworkSubgraphMetadata {
    pub id: IntId,
    pub geohash: Option<String>,
    pub indexer_url: Option<String>,
    pub staked_tokens: BigDecimal,
    pub allocated_tokens: BigDecimal,
    pub locked_tokens: BigDecimal,
    pub query_fees_collected: BigDecimal,
    pub query_fee_rebates: BigDecimal,
    pub rewards_earned: BigDecimal,
    pub indexer_indexing_rewards: BigDecimal,
    pub delegator_indexing_rewards: BigDecimal,
    pub last_updated_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = networks)]
pub struct NewNetwork {
    pub name: String,
    pub caip2: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = indexers)]
pub struct NewIndexer {
    pub address: IndexerAddress,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Queryable, Serialize)]
pub struct SgDeployment {
    pub id: IntId,
    pub cid: String,
    pub name: Option<String>,
    pub network_id: IntId,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = sg_deployments)]
pub struct NewSgDeployment {
    pub ipfs_cid: String,
    pub network: IntId,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = live_pois)]
pub struct NewLivePoi {
    pub poi_id: IntId,
    pub sg_deployment_id: IntId,
    pub indexer_id: IntId,
}

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default)]
#[diesel(sql_type = Jsonb)]
pub struct DivergingBlock {
    pub block_number: i64,
    pub block_hash: Option<String>,
    pub proof_of_indexing1: String,
    pub proof_of_indexing2: String,
}

impl From<types::DivergingBlock> for DivergingBlock {
    fn from(block: types::DivergingBlock) -> Self {
        Self {
            block_number: block.block.number as i64,
            block_hash: block.block.hash.map(|hash| hash.to_string()),
            proof_of_indexing1: block.proof_of_indexing1.to_string(),
            proof_of_indexing2: block.proof_of_indexing2.to_string(),
        }
    }
}

impl FromSql<Jsonb, Pg> for DivergingBlock {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(serde_json::from_value(value)?)
    }
}

//impl From<Poi> for types::ProofOfIndexing {
//    fn from(poi: Poi) -> Self {
//        Self {
//            allocated_tokens: None,
//            deployment: Deployment {
//                id: poi.sg_deployment.cid.clone(),
//            },
//            hash: poi.poi,
//            block: graphix_common_types::Block {
//                network: Network {
//                    name: "mainnet".to_string(),
//                    caip2: None,
//                },
//                number: poi.block.number as u64,
//                hash: poi.block.hash,
//            },
//            indexer: poi.indexer.into_common_type(None),
//        }
//    }
//}
