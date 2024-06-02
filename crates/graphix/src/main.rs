#![allow(clippy::type_complexity)]

mod bisect;
mod utils;

use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use graphix_indexer_client::{IndexerClient, IndexerId};
use graphix_lib::config::Config;
use graphix_lib::graphql_api::{axum_router, ServerState};
use graphix_lib::indexing_loop::{query_indexing_statuses, query_proofs_of_indexing};
use graphix_lib::{config, metrics, PrometheusExporter};
use graphix_store::{models, PoiLiveness, Store};
use prometheus_exporter::prometheus;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::*;

use crate::bisect::handle_divergence_investigation_requests;

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    info!("Parse options");
    let cli_options = CliOptions::parse();

    info!("Loading configuration file");
    let config = Config::read(&cli_options.config)?;

    info!("Initialize store and running migrations");
    let store = Store::new(&config.database_url).await?;
    info!("Store initialization successful");

    if config.graphql.port != 0 {
        let config = config.clone();
        tokio::spawn(async move {
            // Listen to requests forever.
            axum::serve(
                TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.graphql.port)).await?,
                axum_router(config).await?,
            )
            .await?;

            Result::<(), anyhow::Error>::Ok(())
        });
    }

    let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

    // Prometheus metrics.
    let registry = prometheus::default_registry().clone();
    let _exporter = PrometheusExporter::start(config.prometheus_port, registry.clone()).unwrap();

    info!("Initializing bisect request handler");
    let store_clone = store.clone();
    let (tx_indexers, rx_indexers) = watch::channel(vec![]);
    let ctx = ServerState::new(store_clone.clone(), config.clone());

    {
        let networks: Vec<models::NewNetwork> = config
            .chains
            .iter()
            .map(|(name, config)| models::NewNetwork {
                name: name.clone(),
                caip2: config.caip2.clone(),
            })
            .collect();
        store_clone.create_networks_if_missing(&networks).await?;
    }

    tokio::spawn(async move {
        handle_divergence_investigation_requests(&store_clone, rx_indexers, &ctx)
            .await
            .unwrap()
    });

    loop {
        info!("New main loop iteration");
        info!("Initialize inputs (indexers, indexing statuses etc.)");

        let mut indexers = config::config_to_indexers(config.clone(), metrics()).await?;
        // Different data sources, especially network subgraphs, result in
        // duplicate indexers.
        indexers = deduplicate_indexers(&indexers);

        store.write_indexers(&indexers).await?;

        tx_indexers.send(indexers.clone())?;

        let graph_node_versions =
            graphix_lib::indexing_loop::query_graph_node_versions(&indexers, metrics()).await;
        store.write_graph_node_versions(graph_node_versions).await?;

        let indexing_statuses = query_indexing_statuses(&indexers, metrics()).await;

        info!("Monitor proofs of indexing");
        let pois = query_proofs_of_indexing(indexing_statuses, config.block_choice_policy).await;

        info!(pois = pois.len(), "Finished tracking Pois");

        let write_err = store.write_pois(pois, PoiLiveness::Live).await.err();
        if let Some(err) = write_err {
            error!(error = %err, "Failed to write POIs to database");
        }

        info!(
            sleep_seconds = sleep_duration.as_secs(),
            "Sleeping for a while before next main loop iteration"
        );
        tokio::time::sleep(sleep_duration).await;
    }
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}

fn deduplicate_indexers(indexers: &[Arc<dyn IndexerClient>]) -> Vec<Arc<dyn IndexerClient>> {
    info!(len = indexers.len(), "Deduplicating indexers");
    let mut seen = HashSet::new();
    let mut deduplicated = vec![];
    for indexer in indexers {
        if !seen.contains(&indexer.address()) {
            deduplicated.push(indexer.clone());
            seen.insert(indexer.address());
        }
    }
    info!(
        len = deduplicated.len(),
        delta = indexers.len() - deduplicated.len(),
        "Successfully deduplicated indexers"
    );
    deduplicated
}
