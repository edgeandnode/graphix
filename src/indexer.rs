use anyhow::anyhow;
use graphql_client::{GraphQLQuery, Response};
use tracing::*;

use crate::config::EnvironmentUrls;
use crate::types::BlockPointer;
use crate::{config::EnvironmentConfig, pois::ProofOfIndexing};

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

impl TryInto<IndexingStatus> for indexing_statuses::IndexingStatusesIndexingStatuses {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<IndexingStatus, Self::Error> {
        let chain = self
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
                    hash: block.hash.clone(),
                },
                None => {
                    return Err(anyhow!("deployment has not started indexing yet"));
                }
            },
        };

        Ok(IndexingStatus {
            deployment: SubgraphDeployment(self.subgraph),
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
            hash: self.hash,
        }
    }
}

impl TryInto<ProofOfIndexing> for proofs_of_indexing::ProofsOfIndexingPublicProofsOfIndexing {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ProofOfIndexing, Self::Error> {
        match self.proof_of_indexing {
            Some(proof_of_indexing) => Ok(ProofOfIndexing {
                deployment: SubgraphDeployment(self.deployment.clone()),
                block: BlockPointer {
                    number: self.block.number.parse()?,
                    hash: self.block.hash,
                },
                proof_of_indexing,
            }),
            None => Err(anyhow!(
                "no proof of indexing available for deployment {} at block #{} ({})",
                self.deployment,
                self.block.number,
                self.block.hash
            )),
        }
    }
}

/// Indexer

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indexer {
    pub id: String,
    pub urls: EnvironmentUrls,
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
    pub async fn indexing_statuses<'a>(&self) -> Result<Vec<IndexingStatus>, anyhow::Error> {
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
                "indexer returned indexing status errors"
            );
        }

        // Parse indexing statuses
        let mut statuses = vec![];
        if let Some(data) = response.data {
            for indexing_status in data.indexing_statuses {
                let deployment = indexing_status.subgraph.clone();

                match indexing_status.try_into() {
                    Ok(status) => statuses.push(status),
                    Err(e) => {
                        warn!(
                            url = %self.urls.status.to_string(),
                            %e,
                            %deployment,
                            "failed to parse indexing status, skipping deployment"
                        );
                    }
                }
            }
        }
        Ok(statuses)
    }

    pub async fn proofs_of_indexing(
        &self,
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
                    .map(|result| result.try_into())
                    .filter_map(|result| match result {
                        Ok(v) => Some(v),
                        Err(error) => {
                            let Indexer { id, urls, .. } = &self;
                            let url = urls.status.to_string();
                            warn!(%id, %url, %error);
                            None
                        }
                    })
                    .collect()
            })
            .ok_or_else(|| anyhow!("no proofs of indexing returned"))
    }
}
