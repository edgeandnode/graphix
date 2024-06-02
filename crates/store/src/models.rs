use std::borrow::Cow;

use async_graphql::SimpleObject;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::sql_types::Jsonb;
use diesel::{AsChangeset, AsExpression, FromSqlRow, Insertable, Queryable, Selectable};
use graphix_common_types::{self as types, ApiKeyPermissionLevel};
use graphix_indexer_client::IndexerId;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use types::{BlockHash, IndexerAddress, IpfsCid, PoiBytes};
use uuid::Uuid;

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

#[derive(Queryable, Serialize, Debug, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceInvestigationRequest {
    pub pois: Vec<PoiBytes>,
    pub query_block_caches: bool,
    pub query_eth_call_caches: bool,
    pub query_entity_changes: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKey {
    public_part: Uuid,
    private_part: Uuid,
}

impl ApiKey {
    pub fn generate() -> Self {
        Self {
            public_part: Uuid::new_v4(),
            private_part: Uuid::new_v4(),
        }
    }

    pub fn public_part_as_string(&self) -> String {
        self.public_part.to_string()
    }

    pub fn hash(&self) -> Vec<u8> {
        sha2::Sha256::digest(self.to_string().as_bytes()).to_vec()
    }
}

impl std::str::FromStr for ApiKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        let parts: [&str; 3] = parts.try_into().map_err(|_| "invalid api key format")?;

        if parts[0] != "graphix_api_key" {
            return Err("invalid api key format".to_string());
        }

        let public_part = Uuid::try_parse(parts[1]).map_err(|e| e.to_string())?;
        let private_part = Uuid::try_parse(parts[2]).map_err(|e| e.to_string())?;

        Ok(Self {
            public_part,
            private_part,
        })
    }
}

impl std::fmt::Display for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "graphix_api_key-{}-{}",
            self.public_part.as_simple(),
            self.private_part.as_simple()
        )
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, PartialEq, Eq)]
#[diesel(table_name = networks)]
pub struct Network {
    pub id: IntId,
    pub name: String,
    pub caip2: Option<String>,
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

#[derive(Debug, Clone, async_graphql::SimpleObject)]
pub struct NewlyCreatedApiKey {
    pub api_key: String,
    pub notes: Option<String>,
    pub permission_level: ApiKeyPermissionLevel,
}

#[derive(Debug, Clone, Queryable, Serialize)]
pub struct SgDeployment {
    pub id: IntId,
    pub cid: IpfsCid,
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn api_key_from_str() {
        let api_key = ApiKey::generate();
        let parsed = ApiKey::from_str(&format!("{}", api_key)).unwrap();

        assert_eq!(api_key, parsed);
    }
}
