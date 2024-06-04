#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use graphix_common_types::IndexerAddress;
use graphix_indexer_client::{IndexerClient as IndexerTrait, RealIndexer};
use prometheus::IntCounterVec;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::warn;
use url::Url;

const PAGINATION_SIZE: usize = 100;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A GraphQL client that can query the network subgraph and extract useful
/// data.
///
/// The queries available for this client are oriented towards the needs of
/// Graphix, namely to find high-quality and important indexers and subgraph
/// deployments.
#[derive(Debug, Clone)]
pub struct NetworkSubgraphClient {
    endpoint: Url,
    timeout: Duration,
    client: reqwest::Client,
    // Metrics
    // -------
    public_poi_requests: IntCounterVec,
}

impl NetworkSubgraphClient {
    /// Creates a new [`NetworkSubgraphClient`] with the given endpoint.
    pub fn new(endpoint: Url, public_poi_requests: IntCounterVec) -> Self {
        Self {
            endpoint,
            timeout: DEFAULT_TIMEOUT,
            client: reqwest::Client::new(),
            public_poi_requests,
        }
    }

    /// Sets the timeout for requests to the network subgraph.
    ///
    /// The default timeout is 60 seconds.
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
            let real_indexer = indexer_allocation_data_to_real_indexer(
                IndexerAllocation { indexer },
                self.public_poi_requests.clone(),
            );

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

    pub async fn indexers_by_allocations(
        &self,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<Arc<dyn IndexerTrait>>> {
        let indexers = self
            .paginate::<GraphqlResponseTopIndexers, _>(
                queries::INDEXERS_BY_ALLOCATIONS_QUERY,
                vec![],
                "error(s) querying indexers by allocations from the network subgraph",
                |response_data| response_data.indexers,
                limit,
            )
            .await?;

        let mut indexer_clients: Vec<Arc<dyn IndexerTrait>> = vec![];
        for indexer in indexers {
            if let Some(url) = indexer.url {
                let address = str::parse::<IndexerAddress>(&indexer.id)
                    .map_err(|e| anyhow!("invalid indexer address: {}", e))?;
                let real_indexer = RealIndexer::new(
                    indexer.default_display_name,
                    address,
                    Url::parse(&format!("{}/status", url))?.to_string(),
                    self.public_poi_requests.clone(),
                );
                indexer_clients.push(Arc::new(real_indexer));
            }
        }

        Ok(indexer_clients)
    }

    /// Instantiates a [`RealIndexer`] from the indexer with the given address,
    /// querying the necessary information from the network subgraph.
    pub async fn indexer_by_address(
        &self,
        address: &IndexerAddress,
    ) -> anyhow::Result<Arc<dyn IndexerTrait>> {
        let hex_encoded_addr_json = serde_json::to_value(address).unwrap();
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

        let indexer_data = response_data
            .indexers
            .first()
            .ok_or_else(|| anyhow::anyhow!("No indexer found for address {}", address))?;

        let indexer = RealIndexer::new(
            Some(indexer_data.default_display_name.clone()),
            *address,
            Url::parse(&format!("{}/status", indexer_data.url))?.to_string(),
            self.public_poi_requests.clone(),
        );

        Ok(Arc::new(indexer))
    }

    /// Returns all subgraph deployments, ordered by curation signal amounts.
    pub async fn subgraph_deployments_by_signal(
        &self,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<SubgraphDeploymentWithAllocations>> {
        let subgraph_deployments = self
            .paginate::<GraphqlResponseSgDeployments, _>(
                queries::DEPLOYMENTS_QUERY,
                vec![],
                "error(s) querying deployments from the network subgraph",
                |response_data| response_data.subgraph_deployments,
                limit,
            )
            .await?;

        Ok(subgraph_deployments)
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

        tracing::trace!(timeout = ?self.timeout, endpoint = %self.endpoint, "Sending GraphQL request");

        Ok(self
            .client
            .post(self.endpoint.as_str())
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn paginate<R: DeserializeOwned, T>(
        &self,
        query: impl ToString,
        variables: Vec<(String, serde_json::Value)>,
        error_msg: &str,
        response_items: impl Fn(R) -> Vec<T>,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<T>> {
        let page_size = PAGINATION_SIZE;

        let mut items = vec![];
        loop {
            let mut variables = variables.clone();
            variables.push(("first".to_string(), page_size.into()));
            variables.push(("skip".to_string(), items.len().into()));

            let response_data: R = self
                .graphql_query_no_errors(query.to_string(), variables, error_msg)
                .await?;

            // If we got less than the page size, we're done.
            let page_items = response_items(response_data);
            let no_more_results = page_items.len() < page_size;

            items.extend(page_items);

            if no_more_results {
                break;
            }
            if let Some(limit) = limit {
                if items.len() > limit as usize {
                    items.truncate(limit as usize);
                    break;
                }
            }
        }

        Ok(items)
    }
}

fn indexer_allocation_data_to_real_indexer(
    indexer_allocation: IndexerAllocation,
    public_poi_requests: IntCounterVec,
) -> anyhow::Result<RealIndexer> {
    let name = indexer_allocation.indexer.default_display_name.clone();
    let indexer = indexer_allocation.indexer;
    let address = str::parse(&indexer.id).map_err(|e| anyhow!("invalid indexer address: {}", e))?;
    let mut url: Url = indexer
        .url
        .ok_or_else(|| anyhow!("Indexer without URL"))?
        .parse()?;
    url.set_path("/status");
    Ok(RealIndexer::new(
        name,
        address,
        url.to_string(),
        public_poi_requests,
    ))
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
#[serde(rename_all = "camelCase")]
pub struct IndexerAllocation {
    pub indexer: Indexer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Indexer {
    pub id: String,
    pub default_display_name: Option<String>,
    pub url: Option<String>,
}

mod queries {
    pub const INDEXERS_BY_STAKED_TOKENS_QUERY: &str =
        include_str!("queries/indexers_by_staked_tokens.graphql");
    pub const INDEXERS_BY_ALLOCATIONS_QUERY: &str =
        include_str!("queries/indexers_by_allocations.graphql");
    pub const DEPLOYMENTS_QUERY: &str = include_str!("queries/deployments.graphql");
    pub const INDEXER_BY_ADDRESS_QUERY: &str = include_str!("queries/indexer_by_address.graphql");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network_sg_client_on_ethereum() -> NetworkSubgraphClient {
        NetworkSubgraphClient::new(
            "https://api.thegraph.com/subgraphs/name/graphprotocol/graph-network-mainnet"
                .parse()
                .unwrap(),
            IntCounterVec::new(prometheus::Opts::new("foo", "bar"), &["a", "b"]).unwrap(),
        )
    }

    #[tokio::test]
    async fn short_timeout_always_fails() {
        // We should never be able to get a response back under 1ms. If we do,
        // it means the timeout logic is broken.
        let client = network_sg_client_on_ethereum().with_timeout(Duration::from_millis(1));
        assert!(client.indexers_by_staked_tokens().await.is_err())
    }

    #[tokio::test]
    async fn mainnet_indexers_by_staked_tokens_no_panic() {
        let client = network_sg_client_on_ethereum();
        let indexers = client.indexers_by_staked_tokens().await.unwrap();
        assert!(!indexers.is_empty());
    }

    #[tokio::test]
    async fn mainnet_indexers_by_allocations_no_panic() {
        let client = network_sg_client_on_ethereum();
        let indexers = client.indexers_by_allocations(Some(10)).await.unwrap();
        assert_eq!(indexers.len(), 10);
    }

    #[tokio::test]
    async fn subgraph_deployments_limits() {
        let client = network_sg_client_on_ethereum();

        // Single page.
        let deployments = client
            .subgraph_deployments_by_signal(Some(5))
            .await
            .unwrap();
        assert_eq!(deployments.len(), 5);

        // Multiple pages.
        let deployments = client
            .subgraph_deployments_by_signal(Some(150))
            .await
            .unwrap();
        assert_eq!(deployments.len(), 150);
    }

    #[tokio::test]
    async fn mainnet_fetch_ellipfra() {
        let client = network_sg_client_on_ethereum();
        // ellipfra.eth:
        // htps://thegraph.com/explorer/profile/0x62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a?view=Indexing&chain=mainnet
        let address = str::parse("62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a").unwrap();
        let indexer = client.indexer_by_address(&address).await.unwrap();
        assert_eq!(indexer.address(), address);
    }
}
