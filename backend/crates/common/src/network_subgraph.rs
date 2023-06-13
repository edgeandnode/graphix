#![allow(dead_code)]

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct NetworkSubgraph {
    endpoint: String,
    client: reqwest::Client,
}

impl NetworkSubgraph {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }
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
struct GraphqlResponseData {
    subgraph_deployments: Vec<SubgraphDeployment>,
}

#[derive(Deserialize)]
struct SubgraphDeployment {
    ipfs_hash: String,
    indexer_allocations: Vec<IndexerAllocation>,
}

#[derive(Deserialize)]
struct IndexerAllocation {
    indexer: Indexer,
}

#[derive(Deserialize)]
struct Indexer {
    id: String,
    url: String,
}

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

impl NetworkSubgraph {
    // The `curation_threshold` is denominated in GRT.
    async fn subgraph_deployments(&self) -> anyhow::Result<Vec<SubgraphDeployment>> {
        let request = GraphqlRequest {
            query: DEPLOYMENTS_QUERY.to_string(),
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
        let data_deserialized: GraphqlResponseData = serde_json::from_value(data)?;
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
