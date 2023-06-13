use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use graphql_client::{GraphQLQuery, Response};
use tracing::*;

use crate::{
    config::IndexerUrls,
    prelude::IndexerConfig,
    prometheus_metrics::metrics,
    types::{BlockPointer, IndexingStatus, POIRequest, ProofOfIndexing, SubgraphDeployment},
};

use super::Indexer;

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
        Arc<RealIndexer>,
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
                    hash: Some(block.hash.clone().as_str().try_into()?),
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

impl TryInto<ProofOfIndexing>
    for (
        Arc<RealIndexer>,
        proofs_of_indexing::ProofsOfIndexingPublicProofsOfIndexing,
    )
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ProofOfIndexing, Self::Error> {
        Ok(ProofOfIndexing {
            indexer: self.0,
            deployment: SubgraphDeployment(self.1.deployment.clone()),
            block: BlockPointer {
                number: self.1.block.number.parse()?,
                hash: self
                    .1
                    .block
                    .hash
                    .and_then(|hash| hash.as_str().try_into().ok()),
            },
            proof_of_indexing: self.1.proof_of_indexing.as_str().try_into()?,
        })
    }
}

#[derive(Debug)]
pub struct RealIndexer {
    id: String, // Assumed to be unique accross all indexers
    urls: IndexerUrls,
    client: reqwest::Client,
}

impl RealIndexer {
    #[instrument(skip_all)]
    pub fn new(config: IndexerConfig) -> Self {
        RealIndexer {
            id: config.id,
            urls: config.urls,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Indexer for RealIndexer {
    fn id(&self) -> &str {
        &self.id
    }

    fn address(&self) -> Option<&[u8]> {
        None
    }

    async fn indexing_statuses(self: Arc<Self>) -> Result<Vec<IndexingStatus>, anyhow::Error> {
        let request = IndexingStatuses::build_query(indexing_statuses::Variables);
        let response_raw = self
            .client
            .post(self.urls.status.clone())
            .json(&request)
            .send()
            .await?;

        debug!(
            id = %self.id(),
            response = ?response_raw,
            "Indexer returned a response"
        );
        let response: Response<indexing_statuses::ResponseData> = response_raw.json().await?;

        // Log any errors received for debugging
        if let Some(errors) = response.errors {
            let errors = errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join(",");
            warn!(
                id = %self.id(),
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
                            id = %self.id(),
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
    ) -> Result<Vec<ProofOfIndexing>, anyhow::Error> {
        use proofs_of_indexing::{PublicProofOfIndexingRequest, ResponseData, Variables};

        let mut pois = vec![];

        // Graph Node implements a limit of 10 POI requests per request, so
        // split our requests up accordingly.
        for requests in requests.chunks(10) {
            debug!(
                indexer = %self.id(),
                batch_size = requests.len(),
                "Requesting public PoIs"
            );

            let request = ProofsOfIndexing::build_query(Variables {
                requests: requests
                    .into_iter()
                    .map(|query| PublicProofOfIndexingRequest {
                        deployment: query.deployment.to_string(),
                        block_number: query.block_number.to_string(),
                    })
                    .collect(),
            });
            let raw_response = self
                .client
                .post(self.urls.status.clone())
                .json(&request)
                .send()
                .await?
                .text()
                .await?;

            let response = match serde_json::from_str::<Response<ResponseData>>(&raw_response) {
                Ok(response) => response,
                Err(e) => {
                    return Err(anyhow!(
                        "Response is not JSON, parsing error: `{}`, full response: `{}`",
                        e,
                        raw_response
                    ))
                }
            };

            // Log any errors received for debugging
            if let Some(errors) = response.errors {
                metrics()
                    .public_proofs_of_indexing_requests
                    .get_metric_with_label_values(&[self.id(), "0"])
                    .unwrap()
                    .inc();

                let errors = errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(",");
                warn!(
                    id = %self.id(),
                    %errors,
                    "indexer returned POI errors"
                );
            }

            if let Some(data) = response.data {
                metrics()
                    .public_proofs_of_indexing_requests
                    .get_metric_with_label_values(&[self.id(), "1"])
                    .unwrap()
                    .inc();

                // Parse POI results
                pois.extend(
                    data.public_proofs_of_indexing
                        .into_iter()
                        .map(|result| (self.clone(), result).try_into())
                        .filter_map(|result| match result {
                            Ok(v) => Some(v),
                            Err(error) => {
                                warn!(id = %self.id(), %error);
                                None
                            }
                        }),
                );
            } else {
                warn!("no data present, skipping");
            }
        }

        Ok(pois)
    }
}
