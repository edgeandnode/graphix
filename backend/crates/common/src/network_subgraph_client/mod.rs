#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use tracing::warn;

use crate::config::{IndexerConfig, IndexerUrls};
use crate::prelude::{Indexer as IndexerTrait, RealIndexer};

/// A GraphQL client that can query the network subgraph and extract useful
/// data.
#[derive(Debug, Clone)]
pub struct NetworkSubgraphClient {
    endpoint: String,
    timeout: Duration,
    client: reqwest::Client,
}

impl NetworkSubgraphClient {
    /// Creates a new [`NetworkSubgraphClient`] with the given endpoint.
    pub fn new(endpoint: impl ToString) -> Self {
        const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

        Self {
            endpoint: endpoint.to_string(),
            timeout: DEFAULT_TIMEOUT,
            client: reqwest::Client::new(),
        }
    }

    /// Sets the timeout for requests to the network subgraph.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn indexers_by_staked_tokens(&self) -> anyhow::Result<Vec<Arc<dyn IndexerTrait>>> {
        let response_data: GraphqlResponseTopIndexers = self
            .graphql_query_no_errors(
                queries::INDEXERS_BY_STAKED_TOKENS_QUERY,
                vec![],
                "error(s) querying top indexers from the network subgraph",
            )
            .await?;

        let mut indexers: Vec<Arc<dyn IndexerTrait>> = vec![];
        for indexer in response_data.indexers {
            let indexer_id = indexer.id.clone();
            let real_indexer =
                indexer_allocation_data_to_real_indexer(IndexerAllocation { indexer });

            match real_indexer {
                Ok(indexer) => indexers.push(Arc::new(indexer)),
                Err(e) => warn!(
                    err = %e.to_string(),
                    indexer_id,
                    "Received bad indexer for network subgraph query; ignoring",
                ),
            }
        }

        Ok(indexers)
    }

    pub async fn indexers_by_allocations(&self) -> anyhow::Result<Vec<Arc<dyn IndexerTrait>>> {
        let sg_deployments = self.subgraph_deployments().await?;

        let mut indexers: Vec<Arc<dyn IndexerTrait>> = vec![];
        for deployment in sg_deployments {
            for indexer_allocation in deployment.indexer_allocations {
                let url = indexer_allocation.indexer.url.clone();
                if let Ok(indexer) = indexer_allocation_data_to_real_indexer(indexer_allocation) {
                    indexers.push(Arc::new(indexer));
                } else {
                    warn!(url, "Failed to create indexer from allocation data");
                }
            }
        }

        Ok(indexers)
    }

    /// Instantiates a [`RealIndexer`] from the indexer with the given address,
    /// querying the necessary information from the network subgraph.
    pub async fn indexer_by_address(
        &self,
        address: &[u8],
    ) -> anyhow::Result<Arc<dyn IndexerTrait>> {
        let hex_encoded_addr_json = serde_json::to_value(format!("0x{}", hex::encode(address)))
            .expect("Unable to hex encode address");
        println!("hex_encoded_addr_json: {:?}", hex_encoded_addr_json);
        let response_data: ResponseData = self
            .graphql_query_no_errors(
                queries::INDEXER_BY_ADDRESS_QUERY,
                vec![("id".to_string(), hex_encoded_addr_json)],
                "error(s) querying indexer by address from the network subgraph",
            )
            .await?;

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ResponseData {
            indexers: Vec<IndexerData>,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct IndexerData {
            url: String,
            default_display_name: String,
        }

        let indexer_data = response_data.indexers.first().ok_or_else(|| {
            anyhow::anyhow!("No indexer found for address 0x{}", hex::encode(address))
        })?;

        let indexer = Arc::new(RealIndexer::new(IndexerConfig {
            name: indexer_data.default_display_name.clone(),
            urls: IndexerUrls {
                status: Url::parse(&format!("{}/status", indexer_data.url))?,
            },
        }));

        Ok(indexer)
    }

    /// Returns all subgraph deployments.
    pub async fn subgraph_deployments(
        &self,
    ) -> anyhow::Result<Vec<SubgraphDeploymentWithAllocations>> {
        let response_data: GraphqlResponseSgDeployments = self
            .graphql_query_no_errors(
                queries::DEPLOYMENTS_QUERY,
                vec![],
                "error(s) querying deployments from the network subgraph",
            )
            .await?;

        Ok(response_data.subgraph_deployments)
    }

    /// A wrapper around [`NetworkSubgraphClient::graphql_query`] that requires
    /// no errors in the response, and deserializes the response data into the
    /// given type.
    async fn graphql_query_no_errors<T: DeserializeOwned>(
        &self,
        query: impl ToString,
        variables: Vec<(String, serde_json::Value)>,
        err_msg: &str,
    ) -> anyhow::Result<T> {
        let response = self.graphql_query(query, variables).await?;
        let response_data = response.data.ok_or_else(|| {
            anyhow::anyhow!(
                "{}: {}",
                err_msg,
                serde_json::to_string_pretty(&response.errors.unwrap_or_default())
                    .expect("Unable to encode query errors")
            )
        })?;

        Ok(serde_json::from_value(response_data)?)
    }

    /// Sends a generic GraphQL query to the network subgraph.
    pub async fn graphql_query(
        &self,
        query: impl ToString,
        variables: Vec<(String, serde_json::Value)>,
    ) -> anyhow::Result<GraphqlResponse> {
        let request = GraphqlRequest {
            query: query.to_string(),
            variables: BTreeMap::from_iter(variables),
        };

        tracing::trace!(timeout = ?self.timeout, endpoint = self.endpoint, "Sending GraphQL request");

        Ok(self
            .client
            .post(&self.endpoint)
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

fn indexer_allocation_data_to_real_indexer(
    indexer_allocation: IndexerAllocation,
) -> anyhow::Result<RealIndexer> {
    let indexer = indexer_allocation.indexer;
    let mut url: Url = indexer
        .url
        .ok_or_else(|| anyhow!("Indexer without URL"))?
        .parse()?;
    url.set_path("/status");
    let config = IndexerConfig {
        name: indexer.id,
        urls: IndexerUrls { status: url },
    };
    Ok(RealIndexer::new(config))
}

#[derive(Serialize)]
struct GraphqlRequest {
    query: String,
    variables: BTreeMap<String, serde_json::Value>,
}

/// A generic GraphQL response.
#[derive(Deserialize)]
pub struct GraphqlResponse {
    /// The response data.
    pub data: Option<serde_json::Value>,
    /// The response error data.
    pub errors: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlResponseSgDeployments {
    subgraph_deployments: Vec<SubgraphDeploymentWithAllocations>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlResponseTopIndexers {
    indexers: Vec<Indexer>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphDeploymentWithAllocations {
    pub ipfs_hash: String,
    pub indexer_allocations: Vec<IndexerAllocation>,
}

#[derive(Deserialize, Debug)]
pub struct IndexerAllocation {
    pub indexer: Indexer,
}

#[derive(Debug, Deserialize)]
pub struct Indexer {
    pub id: String,
    pub url: Option<String>,
}

mod queries {
    pub const INDEXERS_BY_STAKED_TOKENS_QUERY: &str =
        include_str!("queries/indexers_by_staked_tokens.graphql");
    pub const DEPLOYMENTS_QUERY: &str = include_str!("queries/deployments.graphql");
    pub const INDEXER_BY_ADDRESS_QUERY: &str = include_str!("queries/indexer_by_address.graphql");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network_sg_client_on_ethereum() -> NetworkSubgraphClient {
        NetworkSubgraphClient::new(
            "https://api.thegraph.com/subgraphs/name/graphprotocol/graph-network-mainnet",
        )
    }

    #[tokio::test]
    async fn short_timeout_always_fails() {
        // We should never be able to get a response back under 1ms. If we do,
        // it means the timeout logic is broken.
        let client = network_sg_client_on_ethereum().with_timeout(Duration::from_millis(1));
        assert!(matches!(client.indexers_by_staked_tokens().await, Err(_)));
    }

    #[tokio::test]
    async fn mainnet_indexers_by_staked_tokens_no_panic() {
        let client = network_sg_client_on_ethereum();
        let indexers = client.indexers_by_staked_tokens().await.unwrap();
        assert!(indexers.len() > 0);
    }

    #[tokio::test]
    async fn mainnet_indexers_by_allocations_no_panic() {
        let client = network_sg_client_on_ethereum();
        let indexers = client.indexers_by_allocations().await.unwrap();
        assert!(indexers.len() > 0);
    }

    #[tokio::test]
    async fn at_least_100_subgraph_deployments() {
        let client = network_sg_client_on_ethereum();
        let deployments = client.subgraph_deployments().await.unwrap();
        println!("n. of deployments: {:?}", deployments.len());
        assert!(deployments.len() >= 100);
    }

    #[tokio::test]
    async fn mainnet_fetch_ellipfra() {
        let client = network_sg_client_on_ethereum();
        // ellipfra.eth:
        // htps://thegraph.com/explorer/profile/0x62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a?view=Indexing&chain=mainnet
        let address = hex::decode("62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a").unwrap();
        let indexer = client.indexer_by_address(&address).await.unwrap();
        assert_eq!(indexer.address(), Some(&address[..]));
    }
}
