use async_graphql::SimpleObject;
use chrono::NaiveDateTime;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::sql_types::Jsonb;
use diesel::{backend, AsChangeset, AsExpression, FromSqlRow, Insertable, Queryable};
use serde::{Deserialize, Serialize};
use types::BlockPointer;

use super::schema::*;
use crate::types;

pub type IntId = i32;
pub type BigIntId = i64;
pub type SgDeploymentCid = String;

#[derive(Queryable, Serialize, Debug)]
pub struct Poi {
    pub id: IntId,
    pub poi: Vec<u8>,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
    pub sg_deployment: SgDeployment,
    pub indexer: IndexerRow,
    pub block: Block,
}

impl Poi {
    pub fn poi_hex(&self) -> String {
        hex::encode(&self.poi)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum IndexerRef<'a> {
    Id(IntId),
    Address(&'a [u8]),
}

#[derive(Insertable, Debug)]
#[diesel(table_name = pois)]
pub struct NewPoi {
    pub poi: Vec<u8>,
    pub created_at: NaiveDateTime,
    pub sg_deployment_id: IntId,
    pub indexer_id: IntId,
    pub block_id: BigIntId,
}

pub trait WritablePoi {
    fn deployment_cid(&self) -> &str;
    fn indexer_id(&self) -> &str;
    fn indexer_address(&self) -> Option<&[u8]>;
    fn block(&self) -> BlockPointer;
    fn proof_of_indexing(&self) -> &[u8];
}

#[derive(Queryable, Debug, Serialize)]
pub struct Block {
    pub(super) id: BigIntId,
    _network_id: IntId,
    pub number: i64,
    pub hash: Vec<u8>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = blocks)]
pub struct NewBlock {
    pub network_id: IntId,
    pub number: i64,
    pub hash: Vec<u8>,
}

#[derive(Debug, Queryable)]
pub struct Indexer {
    pub id: IntId,
    pub name: Option<String>,
    pub address: Option<Vec<u8>>,
    pub created_at: NaiveDateTime,
}

impl From<IndexerRow> for Indexer {
    fn from(row: IndexerRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            address: row.address,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Queryable, Serialize)]
pub struct IndexerRow {
    pub id: IntId,
    pub name: Option<String>,
    pub address: Option<Vec<u8>>,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = indexers)]
pub struct NewIndexer {
    pub address: Option<Vec<u8>>,
    pub name: Option<String>,
}

#[derive(Debug, Queryable, Serialize, SimpleObject)]
pub struct QueriedSgDeployment {
    pub id: SgDeploymentCid,
    pub name: String,
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
    fn from_sql(bytes: backend::RawValue<Pg>) -> diesel::deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(serde_json::from_value(value)?)
    }
}
