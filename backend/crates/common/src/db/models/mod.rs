use super::schema::*;
use crate::types;
use chrono::NaiveDateTime;
use diesel::{
    pg::Pg,
    query_dsl::methods::FilterDsl,
    serialize::Output,
    sql_types::Jsonb,
    types::{FromSql, ToSql},
    Insertable, Queryable,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
pub type IntId = i32;

pub type PoIWithId = WithIntId<PoI>;

#[derive(Debug)]
pub enum Filter<S = String> {
    None,
    Id(IntId),
    Value(S),
}

impl<T> FilterDsl<Filter> for T {
    type Output = Filter<T>;

    fn filter(self, predicate: Filter) -> Self::Output {}
}

struct QueryBuilder {
    limit: Option<u32>,
}

impl QueryBuilder {
    pub fn with_limit(mut self, limit: Option<u32>) -> Self {
        self.limit = limit;
        self
    }
}

#[derive(Debug, Queryable)]
pub struct WithIntId<T> {
    pub id: IntId,
    pub inner: T,
}

#[derive(Debug, Insertable, Queryable)]
#[table_name = "pois"]
struct PoIRow {
    //pub id: IntId,
    pub poi: Vec<u8>,
    pub sg_deployment_id: IntId,
    pub indexer_id: IntId,
    pub block_id: IntId,
    pub created_at: NaiveDateTime,
}

#[derive(Debug)]
#[table_name = "pois"]
pub struct PoI {
    pub id: IntId,
    pub poi: Vec<u8>,
    pub sg_deployment: SgDeployment,
    pub indexer: Indexer,
    pub block_id: IntId,
    pub created_at: NaiveDateTime,
}

impl PoI {
    pub fn poi_hex(&self) -> String {
        hex::encode(&self.poi)
    }
}

#[derive(Debug, Insertable, Queryable)]
pub struct Indexer {
    pub id: IntId,
    pub address: Vec<u8>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable, Queryable)]
pub struct SgDeployment {
    pub id: IntId,
    pub deployment: Vec<u8>,
    pub created_at: NaiveDateTime,
}

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default)]
#[sql_type = "Jsonb"]
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
    fn from_sql(bytes: Option<&[u8]>) -> diesel::deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(serde_json::from_value(value)?)
    }
}

impl ToSql<Jsonb, Pg> for DivergingBlock {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> diesel::serialize::Result {
        let value = serde_json::to_value(self)?;
        <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, out)
    }
}

#[derive(Debug, Insertable, Queryable)]
#[table_name = "poi_divergence_bisect_reports"]
pub struct PoiDivergenceBisectReport {
    pub id: IntId,
    pub poi1_id: IntId,
    pub poi2_id: IntId,
    pub divergence_block_id: IntId,
    pub created_at: NaiveDateTime,
}
