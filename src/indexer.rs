use std::sync::Arc;

use anyhow::anyhow;
use graphql_client::{GraphQLQuery, Response};
use tracing::*;

use crate::config::IndexerUrls;
use crate::types::BlockPointer;
use crate::{config::EnvironmentConfig, proofs_of_indexing::ProofOfIndexing};

use super::types::{IndexingStatus, SubgraphDeployment};

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

impl TryInto<IndexingStatus>
    for (
        Arc<Indexer>,
        indexing_statuses::IndexingStatusesIndexingStatuses,
    )
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<IndexingStatus, Self::Error> {
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

#[derive(Debug)]
pub struct POIRequest {
    pub deployment: SubgraphDeployment,
    pub block: BlockPointer,
}

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

impl TryInto<ProofOfIndexing>
    for (
        Arc<Indexer>,
        proofs_of_indexing::ProofsOfIndexingPublicProofsOfIndexing,
    )
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ProofOfIndexing, Self::Error> {
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Indexer {
    pub id: String,
    pub urls: IndexerUrls,
}

impl Indexer {
    #[instrument(skip(env))]
    pub async fn from_environment(env: &EnvironmentConfig) -> Result<Self, anyhow::Error> {
        Ok(Indexer {
            id: env.id.clone(),
            urls: env.urls.clone(),
        })
    }

    #[instrument]
    pub async fn indexing_statuses<'a>(
        self: Arc<Self>,
    ) -> Result<Vec<IndexingStatus>, anyhow::Error> {
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

    pub async fn proofs_of_indexing(
        self: Arc<Self>,
        requests: Vec<POIRequest>,
    ) -> Result<Vec<ProofOfIndexing>, anyhow::Error> {
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
