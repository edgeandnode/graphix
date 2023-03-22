use std::io::Write;

use chrono::NaiveDateTime;
use diesel::{
    pg::Pg,
    serialize::Output,
    sql_types::Jsonb,
    types::{FromSql, ToSql},
    Insertable, Queryable,
};
use serde::{Deserialize, Serialize};

use crate::types;

use super::schema::*;

#[derive(Debug, Insertable, Queryable)]
#[table_name = "proofs_of_indexing"]
pub struct ProofOfIndexing {
    pub timestamp: NaiveDateTime,
    pub indexer: String,
    pub deployment: String,
    pub block_number: i64,
    pub block_hash: Option<String>,
    pub proof_of_indexing: String,
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
#[table_name = "poi_cross_check_reports"]
pub struct PoiCrossCheckReport {
    pub timestamp: NaiveDateTime,
    pub indexer1: String,
    pub indexer2: String,
    pub deployment: String,
    pub block_number: i64,
    pub block_hash: Option<String>,
    pub proof_of_indexing1: String,
    pub proof_of_indexing2: String,
    pub diverging_block: Option<DivergingBlock>,
}
