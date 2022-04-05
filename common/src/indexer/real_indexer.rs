use core::hash::Hash;
use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use graphql_client::{GraphQLQuery, Response};
use tracing::*;

use crate::{
    config::{EnvironmentConfig, IndexerUrls},
    prelude::Bytes32,
    types::{self as t, BlockPointer},
};

use super::Indexer;

mod queries {
    use graphql_client::GraphQLQuery;

    type BigInt = String;
    type Bytes = String;
    type JSONObject = serde_json::Value;

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "graphql/indexer/schema.gql",
        query_path = "graphql/indexer/queries/indexing-statuses.gql",
        response_derives = "Debug",
        variables_derives = "Debug"
    )]
    pub struct IndexingStatuses;

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "graphql/indexer/schema.gql",
        query_path = "graphql/indexer/queries/pois.gql",
        response_derives = "Debug",
        variables_derives = "Debug"
    )]
    pub struct ProofsOfIndexing;

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "graphql/indexer/schema.gql",
        query_path = "graphql/indexer/queries/poi-debug-data.gql",
        response_derives = "Debug",
        variables_derives = "Debug"
    )]
    pub struct PoiDebugData;
}

/// Deserialization utilities from GraphQL response data to [`crate::types`].
mod deserialize {
    use super::queries::*;
    use super::*;

    pub fn indexing_status(
        indexer: Arc<RealIndexer>,
        statuses: indexing_statuses::IndexingStatusesIndexingStatuses,
    ) -> anyhow::Result<t::IndexingStatus<RealIndexer>> {
        let chain = statuses
            .chains
            .get(0)
            .ok_or_else(|| anyhow!("chain status missing"))?;

        let latest_block = match chain.on {
            indexing_statuses::IndexingStatusesIndexingStatusesChainsOn::EthereumIndexingStatus(
                indexing_statuses::IndexingStatusesIndexingStatusesChainsOnEthereumIndexingStatus {
                    ref latest_block,
                    ..
                },
            ) => match latest_block {
                Some(block) => {
                    let hash: Bytes32 = block.hash.clone().as_str().try_into()?;
                    BlockPointer {
                        number: block.number.parse()?,
                        hash: Some(hash),
                    }
                }
                None => {
                    return Err(anyhow!("deployment has not started indexing yet"));
                }
            },
        };

        Ok(t::IndexingStatus {
            indexer,
            deployment: t::SubgraphDeployment {
                deployment_id: statuses.subgraph,
                network: chain.network.clone(),
            },
            latest_block,
        })
    }

    pub async fn pois(
        indexer: Arc<RealIndexer>,
        pois: proofs_of_indexing::ProofsOfIndexingPublicProofsOfIndexing,
        network: String,
    ) -> anyhow::Result<t::ProofOfIndexing<RealIndexer>> {
        let block_number = pois.block.number.parse()?;
        let hash: Option<Bytes32> = pois
            .block
            .hash
            .and_then(|hash| hash.as_str().try_into().ok());

        let debug_data = if let Some(ref hash) = hash {
            indexer
                .clone()
                .poi_debug_data(
                    network.as_str(),
                    pois.deployment.as_str(),
                    block_number,
                    &hash.0[..],
                )
                .await?
        } else {
            t::PoiDebugData::empty()
        };

        let block = BlockPointer {
            number: block_number,
            hash,
        };

        // Parse POI results
        Ok(t::ProofOfIndexing {
            indexer,
            deployment: t::SubgraphDeployment {
                deployment_id: pois.deployment.clone(),
                network,
            },
            block,
            proof_of_indexing: pois.proof_of_indexing.as_str().try_into()?,
            debug_data,
        })
    }

    pub fn poi_debug_data(
        block_contents: Option<serde_json::Value>,
        entity_changes: poi_debug_data::PoiDebugDataEntityChangesInBlock,
        calls: Vec<poi_debug_data::PoiDebugDataCachedEthereumCalls>,
    ) -> anyhow::Result<t::PoiDebugData> {
        let block_contents = block_contents.unwrap_or(serde_json::Value::Null);
        let entity_deletions = entity_changes
            .deletions
            .into_iter()
            .map(|deletion| (deletion.type_, deletion.entities))
            .collect();
        let entity_updates = entity_changes
            .updates
            .into_iter()
            .map(|update| (update.type_, update.entities))
            .collect();
        let cached_calls = calls
            .into_iter()
            .map(|c| {
                Ok(t::CachedEthereumCall {
                    id_hash: parse_hex(c.id_hash)?,
                    return_value: parse_hex(c.return_value)?,
                    contract_address: parse_hex(c.contract_address)?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(t::PoiDebugData {
            block_contents,
            entity_deletions,
            entity_updates,
            cached_calls,
        })
    }

    fn parse_hex(s: impl AsRef<str>) -> anyhow::Result<Vec<u8>> {
        Ok(hex::decode(s.as_ref().trim_start_matches("0x"))?)
    }
}

/// Indexer

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct RealIndexer {
    pub id: String,
    pub urls: IndexerUrls,
}

impl RealIndexer {
    #[instrument(skip(env))]
    pub fn new(env: &EnvironmentConfig) -> Result<Self, anyhow::Error> {
        Ok(Self {
            id: env.id.clone(),
            urls: env.urls.clone(),
        })
    }

    async fn query<T, F>(
        &self,
        vars: T::Variables,
        err_log: F,
    ) -> anyhow::Result<Option<T::ResponseData>>
    where
        T: GraphQLQuery,
        F: Fn(&Self, Vec<graphql_client::Error>),
    {
        let client = reqwest::Client::new();
        let request = T::build_query(vars);
        let response: Response<T::ResponseData> = client
            .post(self.urls.status.clone())
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        // Log any errors received for debugging
        if let Some(e) = response.errors {
            err_log(self, e);
        }

        Ok(response.data)
    }
}

#[async_trait]
impl Indexer for RealIndexer {
    fn id(&self) -> &String {
        &self.id
    }

    fn urls(&self) -> &IndexerUrls {
        &self.urls
    }

    #[instrument]
    async fn indexing_statuses(self: Arc<Self>) -> anyhow::Result<Vec<t::IndexingStatus<Self>>> {
        let data = self
            .query::<queries::IndexingStatuses, _>(
                queries::indexing_statuses::Variables,
                |self, errors| {
                    let errors = errors
                        .into_iter()
                        .map(|e| e.message)
                        .collect::<Vec<_>>()
                        .join(",");
                    warn!(
                        url = %self.urls.status.to_string(),
                        %errors,
                        "Indexer returned indexing status errors"
                    );
                },
            )
            .await?;

        // Parse indexing statuses
        let mut statuses = vec![];
        if let Some(data) = data {
            for indexing_status in data.indexing_statuses {
                let deployment = indexing_status.subgraph.clone();

                match deserialize::indexing_status(self.clone(), indexing_status) {
                    Ok(status) => statuses.push(status),
                    Err(e) => {
                        warn!(
                            url = %self.urls.status.to_string(),
                            %e,
                            %deployment,
                            "Failed to parse indexing status, skipping deployment"
                        );
                    }
                }
            }
        }
        Ok(statuses)
    }

    async fn poi_debug_data(
        self: Arc<Self>,
        network: &str,
        subgraph: &str,
        block_number: u64,
        block_hash: &[u8],
    ) -> anyhow::Result<t::PoiDebugData> {
        let vars = queries::poi_debug_data::Variables {
            network: network.to_string(),
            subgraph: subgraph.to_string(),
            block_number: block_number as _,
            block_hash: hex::encode(block_hash),
        };
        let data = self
            .query::<queries::PoiDebugData, _>(vars, |self, errors| {
                let errors = errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join(",");
                warn!(
                    url = %self.urls.status.to_string(),
                    %errors,
                    "Indexer returned errors when fetching POI debug data"
                );
            })
            .await?
            .ok_or(anyhow!("Missing POI debug data"))?;

        Ok(deserialize::poi_debug_data(
            data.block_data,
            data.entity_changes_in_block,
            data.cached_ethereum_calls.unwrap_or(vec![]),
        )?)
    }

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<t::POIRequest>,
    ) -> Result<Vec<t::ProofOfIndexing<Self>>, anyhow::Error> {
        let network_name_by_deployment: HashMap<String, String> = requests
            .iter()
            .map(|r| {
                (
                    r.deployment.deployment_id.clone(),
                    r.deployment.network.clone(),
                )
            })
            .collect();

        let vars = queries::proofs_of_indexing::Variables {
            requests: requests
                .into_iter()
                .map(
                    |query| queries::proofs_of_indexing::PublicProofOfIndexingRequest {
                        deployment: query.deployment.deployment_id,
                        blockNumber: query.block_number.to_string(),
                    },
                )
                .collect(),
        };
        let data = self
            .query::<queries::ProofsOfIndexing, _>(vars, |self, errors| {
                let errors = errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(",");
                warn!(
                    id = %self.id,
                    url = %self.urls.status.to_string(),
                    %errors,
                    "indexer returned POI errors"
                );
            })
            .await?
            .ok_or(anyhow!("no proofs of indexing returned"))?;

        let pois = vec![];
        for data in data.public_proofs_of_indexing {
            let network = network_name_by_deployment[&data.deployment].clone();
            let result = deserialize::pois(self.clone(), data, network).await;

            match result {
                Ok(v) => Some(v),
                Err(error) => {
                    warn!(id = %self.id, url = %self.urls.status.to_string(), %error);
                    None
                }
            };
        }

        Ok(pois)
    }
}
