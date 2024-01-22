use std::borrow::Cow;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use reqwest::Url;
use serde::{Deserialize, Deserializer};
use tracing::{info, warn};

use crate::block_choice::BlockChoicePolicy;
use crate::indexer::{Indexer, IndexerId, IndexerInterceptor, RealIndexer};
use crate::network_subgraph_client::NetworkSubgraphClient;

/// A [`serde`]-compatible representation of Graphix's YAML configuration file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub database_url: String,
    #[serde(default = "Config::default_prometheus_port")]
    pub prometheus_port: u16,
    pub sources: Vec<ConfigSource>,
    #[serde(default)]
    pub block_choice_policy: BlockChoicePolicy,

    #[serde(default = "Config::default_polling_period_in_seconds")]
    pub polling_period_in_seconds: u64,
}

impl Config {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        serde_yaml::from_reader(file).context("invalid config file")
    }

    pub fn indexers(&self) -> Vec<IndexerConfig> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                ConfigSource::Indexer(config) => Some(config),
                _ => None,
            })
            .cloned()
            .collect()
    }

    pub fn indexers_by_address(&self) -> Vec<IndexerByAddressConfig> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                ConfigSource::IndexerByAddress(config) => Some(config),
                _ => None,
            })
            .cloned()
            .collect()
    }

    pub fn interceptors(&self) -> Vec<InterceptorConfig> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                ConfigSource::Interceptor(config) => Some(config),
                _ => None,
            })
            .cloned()
            .collect()
    }

    pub fn network_subgraphs(&self) -> Vec<NetworkSubgraphConfig> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                ConfigSource::NetworkSubgraph(config) => Some(config),
                _ => None,
            })
            .cloned()
            .collect()
    }

    fn default_polling_period_in_seconds() -> u64 {
        120
    }

    fn default_prometheus_port() -> u16 {
        9184
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct IndexerUrls {
    #[serde(deserialize_with = "deserialize_url")]
    pub status: Url,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerConfig {
    pub name: Option<String>,
    pub address: Option<Vec<u8>>,
    pub urls: IndexerUrls,
}

impl IndexerId for IndexerConfig {
    fn address(&self) -> Option<&[u8]> {
        self.address.as_deref()
    }

    fn name(&self) -> Option<Cow<String>> {
        self.name.as_ref().map(Cow::Borrowed)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerByAddressConfig {
    #[serde(deserialize_with = "deserialize_hexstring")]
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubgraphConfig {
    pub endpoint: String,
    /// What query out of several available ones to use to fetch the list of
    /// indexers from the network subgraph?
    #[serde(default)]
    pub query: NetworkSubgraphQuery,
    pub stake_threshold: f64,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkSubgraphQuery {
    #[default]
    ByAllocations,
    ByStakedTokens,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterceptorConfig {
    pub name: String,
    pub target: String,
    pub poi_byte: u8,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConfigSource {
    Indexer(IndexerConfig),
    IndexerByAddress(IndexerByAddressConfig),
    Interceptor(InterceptorConfig),
    NetworkSubgraph(NetworkSubgraphConfig),
}

fn deserialize_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(serde::de::Error::custom)
}

fn deserialize_hexstring<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if !s.starts_with("0x") {
        return Err(serde::de::Error::custom("hexstring must start with 0x"));
    }
    hex::decode(&s[2..]).map_err(serde::de::Error::custom)
}

pub async fn config_to_indexers(config: Config) -> anyhow::Result<Vec<Arc<dyn Indexer>>> {
    let mut indexers: Vec<Arc<dyn Indexer>> = vec![];

    // First, configure all the real, static indexers.
    for config in config.indexers() {
        info!(indexer_id = %config.id(), "Configuring indexer");
        indexers.push(Arc::new(RealIndexer::new(config.clone())));
    }

    // Then, configure the network subgraphs, if required, resulting in "dynamic"
    // indexers.
    for config in config.network_subgraphs() {
        info!(endpoint = %config.endpoint, "Configuring network subgraph");
        let network_subgraph = NetworkSubgraphClient::new(config.endpoint.clone());
        let network_subgraph_indexers_res = match config.query {
            NetworkSubgraphQuery::ByAllocations => {
                network_subgraph.indexers_by_allocations(config.limit).await
            }
            NetworkSubgraphQuery::ByStakedTokens => {
                network_subgraph.indexers_by_staked_tokens().await
            }
        };
        if let Ok(mut network_subgraph_indexers) = network_subgraph_indexers_res {
            if let Some(limit) = config.limit {
                network_subgraph_indexers.truncate(limit as usize);
            }

            indexers.extend(network_subgraph_indexers);
        } else {
            warn!(
                endpoint = %config.endpoint,
                error = %network_subgraph_indexers_res.as_ref().unwrap_err(),
                "Failed to configure network subgraph"
            );
        }
    }

    info!(
        indexer_count = indexers.len(),
        "Configured all network subgraphs"
    );

    // Then, configure indexers by address, which requires access to a network subgraph.
    for indexer_config in config.indexers_by_address() {
        // FIXME: when looking up indexers by address, we don't really know
        // which network subgraph to use for the lookup. Should this be
        // indicated inside the data source's configuration? Should we try all
        // network subgraphs until one succeeds?
        let network_subgraph = NetworkSubgraphClient::new(
            config
                .network_subgraphs()
                .first()
                .ok_or_else(|| anyhow::anyhow!("indexer by address requires a network subgraph"))?
                .endpoint
                .clone(),
        );
        let indexer = network_subgraph
            .indexer_by_address(&indexer_config.address)
            .await?;
        indexers.push(indexer);
    }

    // Finally, configure all the interceptors, referring to the real, static
    // indexers by ID.
    for config in config.interceptors() {
        info!(interceptor_id = %config.name, "Configuring interceptor");
        let target = indexers
            .iter()
            .find(|indexer| indexer.id() == config.target)
            .expect("interceptor target indexer not found");
        indexers.push(Arc::new(IndexerInterceptor::new(
            target.clone(),
            config.poi_byte,
        )));
    }

    Ok(indexers)
}
