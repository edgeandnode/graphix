use core::hash::Hash;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use graphql_client::{GraphQLQuery, Response};
use tracing::*;

use crate::{
    config::{EnvironmentConfig, IndexerUrls},
    types::{BlockPointer, IndexingStatus, ProofOfIndexing, SubgraphDeployment},
};

use super::{Indexer, POIRequest};

type BigInt = String;
type Bytes = String;

/// Indexing Statuses

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/indexer/schema.gql",
    query_path = "graphql/indexer/indexing-statuses.gql",
    response_derives = "Debug",
    variables_derives = "Debug"
)]
struct IndexingStatuses;

impl TryInto<IndexingStatus<RealIndexer>>
    for (
        Arc<RealIndexer>,
        indexing_statuses::IndexingStatusesIndexingStatuses,
    )
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<IndexingStatus<RealIndexer>, Self::Error> {
        let chain = self
            .1
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
                Some(block) => BlockPointer {
                    number: block.number.parse()?,
                    hash: block.hash.clone().into(),
                },
                None => {
                    return Err(anyhow!("deployment has not started indexing yet"));
                }
            },
        };

        Ok(IndexingStatus {
            indexer: self.0,
            deployment: SubgraphDeployment(self.1.subgraph),
            network: chain.network.clone(),
            latest_block,
        })
    }
}

/// POIs

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/indexer/schema.gql",
    query_path = "graphql/indexer/pois.gql",
    response_derives = "Debug",
    variables_derives = "Debug"
)]
struct ProofsOfIndexing;

impl Into<proofs_of_indexing::BlockInput> for BlockPointer {
    fn into(self) -> proofs_of_indexing::BlockInput {
        proofs_of_indexing::BlockInput {
            number: self.number.to_string(),
            hash: self.hash.into(),
        }
    }
}

impl TryInto<ProofOfIndexing<RealIndexer>>
    for (
        Arc<RealIndexer>,
        proofs_of_indexing::ProofsOfIndexingPublicProofsOfIndexing,
    )
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ProofOfIndexing<RealIndexer>, Self::Error> {
        match self.1.proof_of_indexing {
            Some(proof_of_indexing) => Ok(ProofOfIndexing {
                indexer: self.0,
                deployment: SubgraphDeployment(self.1.deployment.clone()),
                block: BlockPointer {
                    number: self.1.block.number.parse()?,
                    hash: self.1.block.hash.into(),
                },
                proof_of_indexing: proof_of_indexing.into(),
            }),
            None => Err(anyhow!(
                "no proof of indexing available for deployment {} at block #{} ({})",
                self.1.deployment,
                self.1.block.number,
                self.1.block.hash
            )),
        }
    }
}

/// Indexer

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
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
}

impl Hash for RealIndexer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.urls.hash(state);
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
    async fn indexing_statuses(
        self: Arc<Self>,
    ) -> Result<Vec<IndexingStatus<Self>>, anyhow::Error> {
        let client = reqwest::Client::new();
        let request = IndexingStatuses::build_query(indexing_statuses::Variables);
        let response: Response<indexing_statuses::ResponseData> = client
            .post(self.urls.status.clone())
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        // Log any errors received for debugging
        if let Some(errors) = response.errors {
            let errors = errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join(",");
            warn!(
                url = %self.urls.status.to_string(),
                %errors,
                "Indexer returned indexing status errors"
            );
        }

        // Parse indexing statuses
        let mut statuses = vec![];
        if let Some(data) = response.data {
            for indexing_status in data.indexing_statuses {
                let deployment = indexing_status.subgraph.clone();

                match (self.clone(), indexing_status).try_into() {
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

    async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<POIRequest>,
    ) -> Result<Vec<ProofOfIndexing<Self>>, anyhow::Error> {
        let client = reqwest::Client::new();
        let request = ProofsOfIndexing::build_query(proofs_of_indexing::Variables {
            requests: requests
                .into_iter()
                .map(|query| proofs_of_indexing::ProofOfIndexingRequest {
                    deployment: query.deployment.to_string(),
                    block: query.block.into(),
                })
                .collect(),
        });
        let response: Response<proofs_of_indexing::ResponseData> = client
            .post(self.urls.status.clone())
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        // Log any errors received for debugging
        if let Some(errors) = response.errors {
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
        }

        // Parse POI results
        response
            .data
            .map(|data| {
                data.public_proofs_of_indexing
                    .into_iter()
                    .map(|result| (self.clone(), result).try_into())
                    .filter_map(|result| match result {
                        Ok(v) => Some(v),
                        Err(error) => {
                            warn!(id = %self.id, url = %self.urls.status.to_string(), %error);
                            None
                        }
                    })
                    .collect()
            })
            .ok_or_else(|| anyhow!("no proofs of indexing returned"))
    }
}
