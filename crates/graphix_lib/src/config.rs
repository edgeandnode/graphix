//! Graphix configuration parsing and validation.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use graphix_common_types::IndexerAddress;
use graphix_indexer_client::{IndexerClient, IndexerId, IndexerInterceptor, RealIndexer};
use graphix_network_sg_client::NetworkSubgraphClient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use url::Url;

use crate::block_choice::BlockChoicePolicy;
use crate::PrometheusMetrics;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphQlConfig {
    /// The port on which the GraphQL API server should listen. Set it to 0 to
    /// disable the API server entirely.
    #[serde(default = "Config::default_graphql_api_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BlockExplorerUrlTemplateForBlock(String);

impl BlockExplorerUrlTemplateForBlock {
    pub fn url_for_block(&self, block_height: u64) -> String {
        self.0.replace("{block}", block_height.to_string().as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainSpeedConfig {
    pub sample_block_height: u64,
    /// In RFC 3339 format.
    pub sample_timestamp: chrono::DateTime<chrono::Utc>,
    pub avg_block_time_in_msecs: u64,
}

/// Chain-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainConfig {
    pub caip2: Option<String>,
    /// Specifies an approximation of the standard block time for this chain, to
    /// approximate block timestamps.
    #[serde(flatten, default)]
    pub speed: Option<ChainSpeedConfig>,
    /// URL to a block explorer for this chain, with `{block}` as a placeholder
    /// for the block number.
    #[serde(default)]
    pub block_explorer_url_template_for_block: Option<BlockExplorerUrlTemplateForBlock>,
}

/// A [`serde`]-compatible representation of Graphix's YAML configuration file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// GraphQL API configuration.
    pub graphql: GraphQlConfig,
    /// The URL of the PostgreSQL database to use.
    pub database_url: String,
    /// The port on which the Prometheus exporter should listen.
    #[serde(default = "Config::default_prometheus_port")]
    pub prometheus_port: u16,
    /// Chain-specific configuration.
    #[serde(default)]
    pub chains: HashMap<String, ChainConfig>,

    // Indexing options
    // ----------------
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

    fn default_graphql_api_port() -> u16 {
        3030
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexerConfig {
    pub name: Option<String>,
    pub address: IndexerAddress,
    pub index_node_endpoint: Url,
}

impl IndexerId for IndexerConfig {
    fn address(&self) -> IndexerAddress {
        self.address
    }

    fn name(&self) -> Option<Cow<str>> {
        match &self.name {
            Some(name) => Some(Cow::Borrowed(name)),
            None => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexerByAddressConfig {
    pub address: IndexerAddress,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NetworkSubgraphQuery {
    #[default]
    ByAllocations,
    ByStakedTokens,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterceptorConfig {
    pub name: String,
    pub target: IndexerAddress,
    pub poi_byte: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConfigSource {
    Indexer(IndexerConfig),
    IndexerByAddress(IndexerByAddressConfig),
    Interceptor(InterceptorConfig),
    NetworkSubgraph(NetworkSubgraphConfig),
}

pub async fn config_to_indexers(
    config: Config,
    metrics: &PrometheusMetrics,
) -> anyhow::Result<Vec<Arc<dyn IndexerClient>>> {
    let mut indexers: Vec<Arc<dyn IndexerClient>> = vec![];

    // First, configure all the real, static indexers.
    for config in config.indexers() {
        info!(indexer_address = %config.address_string(), "Configuring indexer");
        indexers.push(Arc::new(RealIndexer::new(
            config.name().map(|s| s.into_owned()),
            config.address(),
            config.index_node_endpoint.to_string(),
            metrics.public_proofs_of_indexing_requests.clone(),
        )));
    }

    // Then, configure the network subgraphs, if required, resulting in "dynamic"
    // indexers.
    for config in config.network_subgraphs() {
        info!(endpoint = %config.endpoint, "Configuring network subgraph");
        let network_subgraph = NetworkSubgraphClient::new(
            config.endpoint.as_str().parse()?,
            metrics.public_proofs_of_indexing_requests.clone(),
        );
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
                .parse()?,
            metrics.public_proofs_of_indexing_requests.clone(),
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
            .find(|indexer| indexer.address() == config.target)
            .expect("interceptor target indexer not found");
        indexers.push(Arc::new(IndexerInterceptor::new(
            target.clone(),
            config.poi_byte,
        )));
    }

    Ok(indexers)
}
