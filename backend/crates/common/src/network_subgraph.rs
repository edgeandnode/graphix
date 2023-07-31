#![allow(dead_code)]

use anyhow::anyhow;
use reqwest::Url;
use serde_derive::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tracing::warn;

use crate::{
    config::{IndexerConfig, IndexerUrls},
    prelude::{Indexer as IndexerTrait, RealIndexer},
};

#[derive(Debug, Clone)]
pub struct NetworkSubgraph {
    endpoint: String,
    client: reqwest::Client,
}

impl NetworkSubgraph {
    const INDEXERS_BY_STAKED_TOKENS_QUERY: &str = r#"
        {
          indexers(orderBy: stakedTokens) {
            id
            url
          }
        }
    "#;

    const DEPLOYMENTS_QUERY: &str = r#"
        {
          subgraphDeployments(where: { indexerAllocations_: { status_in: [Active]  }}, orderBy:stakedTokens) {
            ipfsHash
            indexerAllocations(orderBy:allocatedTokens) {
              indexer {
                id
                url
              }
            }
          }
        }
    "#;

    const INDEXER_BY_ADDRESS_QUERY: &str = r#"
        {
          indexers(where: { id: $id }) {
            url
            defaultDisplayName
          }
        }
    "#;

    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    pub async fn indexers_by_staked_tokens(&self) -> anyhow::Result<Vec<Arc<dyn IndexerTrait>>> {
        let request = GraphqlRequest {
            query: Self::INDEXERS_BY_STAKED_TOKENS_QUERY.to_string(),
            variables: BTreeMap::new(), // Our query doesn't require any variables.
        };

        let res: GraphqlResponse = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let errors = res.errors.unwrap_or_default();
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "error(s) querying top indexers from the network subgraph: {}",
                serde_json::to_string(&errors)?
            ));
        }

        // Unwrap: A response that has no errors must contain data.
        let data = res.data.unwrap();
        let data_deserialized: GraphqlResponseTopIndexers = serde_json::from_value(data)?;

        let mut indexers: Vec<Arc<dyn IndexerTrait>> = vec![];
        for indexer in data_deserialized.indexers {
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
        let request = GraphqlRequest {
            query: Self::INDEXER_BY_ADDRESS_QUERY.to_string(),
            variables: BTreeMap::from_iter(vec![(
                "id".to_string(),
                serde_json::to_value(hex::encode(address))?,
            )]),
        };

        let res: GraphqlResponse = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let errors = res.errors.unwrap_or_default();
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "error(s) querying indexer by address from the network subgraph: {}",
                serde_json::to_string(&errors)?
            ));
        }

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

        // Unwrap: A response that has no errors must contain data.
        let data = res.data.unwrap();
        println!("data: {:?}", data);
        let data_deserialized: ResponseData = serde_json::from_value(data)?;
        let indexer_data = data_deserialized.indexers.first().ok_or_else(|| {
            anyhow::anyhow!("No indexer found for address {}", hex::encode(address))
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
        let request = GraphqlRequest {
            query: Self::DEPLOYMENTS_QUERY.to_string(),
            variables: BTreeMap::new(), // Our query doesn't require any variables.
        };

        let res: GraphqlResponse = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let errors = res.errors.unwrap_or_default();
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "error(s) querying deployments from the network subgraph: {}",
                serde_json::to_string(&errors)?
            ));
        }

        // Unwrap: A response that has no errors must contain data.
        let data = res.data.unwrap();
        let data_deserialized: GraphqlResponseSgDeployments = serde_json::from_value(data)?;
        //let page: Vec<SubgraphDeployment> = page
        //    .into_iter()
        //    .map(|raw_deployment| SubgraphDeployment {
        //        // Unwrap: The id returned by the subgraph is a 32 byte long hexadecimal.
        //        id: <[u8; 32]>::try_from(
        //            hex::decode(raw_deployment.id.trim_start_matches("0x")).unwrap(),
        //        )
        //        .unwrap(),
        //        signal_amount: u128::from_str(&raw_deployment.stakedTokens).unwrap(),
        //        deny: raw_deployment.deniedAt > 0,
        //    })
        //    .collect();

        //trace!(this.logger, "deployments page"; "page_size" => page.len());

        Ok(data_deserialized.subgraph_deployments)
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
    // FIXME: we're unable to connect to indexers over HTTPS inside
    // docker-compose for now.
    url.set_scheme("http")
        .map_err(|_| anyhow::anyhow!("unable to set scheme"))?;
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

    // Panics if `cid` version is not v0.
    pub fn cid_v0_to_bytes32(cid: &Cid) -> [u8; 32] {
        assert!(cid.version() == tiny_cid::Version::V0);
        let cid_bytes = cid.to_bytes();

        // A CIDv0 in byte form is 34 bytes long, starting with 0x1220.
        assert_eq!(cid_bytes.len(), 34);

        let mut bytes: [u8; 32] = [0; 32];
        bytes.copy_from_slice(&cid_bytes[2..]);
        bytes
    }
}
