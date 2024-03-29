use std::borrow::Cow;

use async_graphql::SimpleObject;
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
pub struct FailedQuery {
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
    #[serde(skip)]
    pub created_at: NaiveDateTime,
    pub sg_deployment: SgDeployment,
    pub indexer: Indexer,
    pub block: Block,
}

#[derive(Selectable, Insertable, Debug)]
#[diesel(table_name = indexer_versions)]
pub struct NewIndexerVersion {
    pub indexer_id: IntId,
    pub error: Option<String>,
    pub version_string: Option<String>,
    pub version_commit: Option<String>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = indexer_versions)]
pub struct IndexerVersion {
    pub id: IntId,
    pub indexer_id: IntId,
    pub error: Option<String>,
    pub version_string: Option<String>,
    pub version_commit: Option<String>,
    pub created_at: NaiveDateTime,
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

#[derive(Queryable, Debug, Serialize)]
pub struct Block {
    pub(super) id: BigIntId,
    _network_id: IntId,
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

#[derive(Debug, Queryable, Selectable, Serialize)]
pub struct Indexer {
    pub id: IntId,
    pub name: Option<String>,
    pub address: IndexerAddress,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
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

#[derive(Debug, Insertable)]
#[diesel(table_name = indexers)]
pub struct NewIndexer {
    pub address: IndexerAddress,
    pub name: Option<String>,
}

/// A subgraph deployment that is monitored by Graphix.
#[derive(Debug, Queryable, Serialize, SimpleObject)]
pub struct QueriedSgDeployment {
    /// IPFS CID of the subgraph deployment.
    pub id: SgDeploymentCid,
    /// Human-readable name of the subgraph deployment, if present.
    pub name: Option<String>,
    /// Network name of the subgraph deployment.
    pub network_name: String,
}

#[derive(Debug, Queryable, Serialize)]
pub struct SgDeployment {
    pub id: IntId,
    pub cid: String,
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

impl From<Indexer> for graphix_common_types::Indexer {
    fn from(indexer: Indexer) -> Self {
        Self {
            address: indexer.address(),
            name: indexer.name,
            version: None,          // TODO
            allocated_tokens: None, // TODO: we don't store this in the db yet
        }
    }
}

impl From<Poi> for types::ProofOfIndexing {
    fn from(poi: Poi) -> Self {
        Self {
            allocated_tokens: None,
            deployment: Deployment {
                id: poi.sg_deployment.cid.clone(),
            },
            hash: poi.poi,
            block: graphix_common_types::Block {
                network: Network {
                    name: "mainnet".to_string(),
                    caip2: None,
                },
                number: poi.block.number as u64,
                hash: poi.block.hash,
            },
            indexer: poi.indexer.into(),
        }
    }
}
