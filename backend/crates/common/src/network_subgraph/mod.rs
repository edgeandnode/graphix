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

#[derive(Debug, Clone)]
pub struct NetworkSubgraph {
    endpoint: String,
    timeout: Duration,
    client: reqwest::Client,
}

impl NetworkSubgraph {
    /// Creates a new [`NetworkSubgraph`] with the given endpoint.
    pub fn new(endpoint: String) -> Self {
        const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

        Self {
            endpoint,
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

    pub async fn indexer_by_address(
        &self,
        address: &[u8],
    ) -> anyhow::Result<Arc<dyn IndexerTrait>> {
        let hex_encoded_addr_json = serde_json::to_value(format!("0x{}", hex::encode(address)))
            .expect("Unable to hex encode address");
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

    // The `curation_threshold` is denominated in GRT.
    pub async fn subgraph_deployments(&self) -> anyhow::Result<Vec<SubgraphDeployment>> {
        let response_data: GraphqlResponseSgDeployments = self
            .graphql_query_no_errors(
                queries::DEPLOYMENTS_QUERY,
                vec![],
                "error(s) querying deployments from the network subgraph",
            )
            .await?;

        Ok(response_data.subgraph_deployments)
    }

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

    async fn graphql_query(
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

#[derive(Deserialize)]
struct GraphqlResponse {
    data: Option<serde_json::Value>,
    errors: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlResponseSgDeployments {
    subgraph_deployments: Vec<SubgraphDeployment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlResponseTopIndexers {
    indexers: Vec<Indexer>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphDeployment {
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

impl NetworkSubgraph {}

mod util {
    use tiny_cid::Cid;

    pub fn bytes32_to_cid_v0(bytes32: [u8; 32]) -> Cid {
        let mut cidv0: [u8; 34] = [0; 34];

        // The start of any CIDv0.
        cidv0[0] = 0x12;
        cidv0[1] = 0x20;

        cidv0[2..].copy_from_slice(&bytes32);

        // Unwrap: We've constructed a valid CIDv0.
        Cid::read_bytes(cidv0.as_ref()).unwrap()
    }

    /// # Panics
    ///
    /// Panics if `cid` version is not `v0`.
    pub fn cid_v0_to_bytes32(cid: &Cid) -> [u8; 32] {
        assert!(cid.version() == tiny_cid::Version::V0);
        let cid_bytes = cid.to_bytes();

        // A CIDv0 in byte form is 34 bytes long, starting with 0x1220.
        assert_eq!(cid_bytes.len(), 34);

        let mut bytes: [u8; 32] = [0; 32];
        bytes.copy_from_slice(&cid_bytes[2..]);
        bytes
    }

    #[cfg(test)]
    mod tests {
        use quickcheck::{Arbitrary, Gen};
        use quickcheck_macros::quickcheck;

        #[derive(Debug, Clone)]
        struct Bytes([u8; 32]);

        impl Arbitrary for Bytes {
            fn arbitrary(g: &mut Gen) -> Self {
                let mut bytes = [0; 32];
                bytes.fill_with(|| Arbitrary::arbitrary(g));
                Self(bytes)
            }
        }

        #[quickcheck]
        fn convert_to_cid_v0_and_back(bytes32: Bytes) {
            let cid = super::bytes32_to_cid_v0(bytes32.0);
            let bytes32_back = super::cid_v0_to_bytes32(&cid);
            assert_eq!(bytes32.0, bytes32_back);
        }
    }
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

    fn mainnet_network_subgraph() -> NetworkSubgraph {
        NetworkSubgraph::new(
            "https://api.thegraph.com/subgraphs/name/graphprotocol/graph-network-mainnet"
                .to_string(),
        )
    }

    #[tokio::test]
    async fn mainnet_indexers_by_staked_tokens_no_panic() {
        let network_sg = mainnet_network_subgraph();
        let indexers = network_sg.indexers_by_staked_tokens().await.unwrap();
        assert!(indexers.len() > 0);
    }

    #[tokio::test]
    async fn mainnet_indexers_by_allocations_no_panic() {
        let network_sg = mainnet_network_subgraph();
        let indexers = network_sg.indexers_by_allocations().await.unwrap();
        assert!(indexers.len() > 0);
    }

    #[tokio::test]
    async fn mainnet_deployments_no_panic() {
        let network_sg = mainnet_network_subgraph();
        let deployments = network_sg.subgraph_deployments().await.unwrap();
        assert!(deployments.len() > 0);
    }

    #[tokio::test]
    #[should_panic] // FIXME
    async fn mainnet_fetch_indexer() {
        let network_sg = mainnet_network_subgraph();
        // ellipfra.eth:
        // https://thegraph.com/explorer/profile/0x62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a?view=Indexing&chain=mainnet
        let addr = hex::decode("62a0bd1d110ff4e5b793119e95fc07c9d1fc8c4a").unwrap();
        let indexer = network_sg.indexer_by_address(&addr).await.unwrap();
        assert_eq!(indexer.address(), Some(&addr[..]));
    }
}
