use reqwest::Url;
use serde::{Deserialize, Deserializer};
use std::{fs::File, path::Path, sync::Arc};
use tracing::{info, instrument};

use crate::{
    network_subgraph::NetworkSubgraph,
    prelude::{interceptor::IndexerInterceptor, Indexer, RealIndexer},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub database_url: String,
    pub sources: Vec<ConfigSource>,
    #[serde(default = "Config::default_polling_period_in_seconds")]
    pub polling_period_in_seconds: u64,
}

impl Config {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let config: Self = serde_yaml::from_reader(file)
            .map_err(|e| anyhow::Error::new(e).context("invalid config file"))?;

        let num_network_subgraph_sources = config
            .sources
            .iter()
            .filter(|c| match c {
                ConfigSource::NetworkSubgraph(_) => true,
                _ => false,
            })
            .count();

        // Validation: there can only be one network subgraph source, at most.
        if num_network_subgraph_sources > 1 {
            anyhow::bail!("there can only be one network subgraph source");
        }

        Ok(config)
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

    pub fn network_subgraph(&self) -> Option<NetworkSubgraphConfig> {
        let network_subgraphs: Vec<NetworkSubgraphConfig> = self
            .sources
            .iter()
            .filter_map(|source| match source {
                ConfigSource::NetworkSubgraph(config) => Some(config),
                _ => None,
            })
            .cloned()
            .collect();

        // This was already checked by [`Config::read`], it's just some
        // defensive programming.
        debug_assert!(network_subgraphs.len() <= 1);
        network_subgraphs.into_iter().next()
    }

    fn default_polling_period_in_seconds() -> u64 {
        120
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
    pub id: String,
    pub urls: IndexerUrls,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubgraphConfig {
    pub endpoint: String,
    pub stake_threshold: f64,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterceptorConfig {
    pub id: String,
    pub target: String,
    pub poi_byte: u8,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConfigSource {
    Indexer(IndexerConfig),
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

#[instrument]
pub async fn config_to_indexers(config: Config) -> anyhow::Result<Vec<Arc<dyn Indexer>>> {
    let mut indexers: Vec<Arc<dyn Indexer>> = vec![];

    let static_indexers = config.indexers();
    let interceptors = config.interceptors();

    // First, configure all the real, static indexers.
    for config in static_indexers {
        info!(indexer_id = %config.id, "Configuring indexer");
        indexers.push(Arc::new(RealIndexer::new(config.clone())));
    }

    // Then, configure the network subgraph, if required, resulting in "dynamic"
    // indexers.
    if let Some(config) = config.network_subgraph() {
        let network_subgraph = NetworkSubgraph::new(config.endpoint);
        let mut network_subgraph_indexers = network_subgraph.indexers().await?;
        if let Some(limit) = config.limit {
            network_subgraph_indexers.truncate(limit as usize);
        }

        info!("Configuring network subgraph");
        indexers.extend(network_subgraph_indexers);
    }

    // Finally, configure all the interceptors, referring to the real, static
    // indexers by ID.
    for config in interceptors {
        info!(interceptor_id = %config.id, "Configuring interceptor");
        let target = indexers
            .iter()
            .find(|indexer| indexer.id() == config.target)
            .expect("interceptor target indexer not found");
        indexers.push(Arc::new(IndexerInterceptor::new(
            config.id,
            target.clone(),
            config.poi_byte,
        )));
    }

    Ok(indexers)
}
