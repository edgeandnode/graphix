#![allow(clippy::type_complexity)]

use std::collections::HashSet;
use std::env;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use graphix_indexer_client::{IndexerClient, IndexerId};
use graphix_lib::bisect::handle_divergence_investigation_requests;
use graphix_lib::config::Config;
use graphix_lib::graphql_api::{axum_router, GraphixState};
use graphix_lib::indexing_loop::{query_indexing_statuses, query_proofs_of_indexing};
use graphix_lib::{config, metrics, CliOptions, PrometheusExporter};
use graphix_store::{models, PoiLiveness, Store};
use prometheus_exporter::prometheus;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::*;

async fn load_config(store: &Store) -> anyhow::Result<Config> {
    info!("Loading configuration from database...");
    let config_json_opt = store.current_config().await?;

    Ok(if let Some(json) = config_json_opt {
        serde_json::from_value(json)?
    } else {
        warn!("Missing configuration; using empty configuration");
        Config::default()
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli_options = CliOptions::parse();

    info!("Initialize store and running migrations");
    let store = Store::new(&cli_options.database_url).await?;
    info!("Store initialization successful");

    let (config_sender, config_receiver) = watch::channel(load_config(&store).await?);

    {
        let config_receiver = config_receiver.clone();
        tokio::spawn(async move {
            axum::serve(
                TcpListener::bind((Ipv4Addr::UNSPECIFIED, cli_options.port)).await?,
                axum_router(&cli_options.database_url, config_receiver).await?,
            )
            .await?;

            Result::<(), anyhow::Error>::Ok(())
        });
    }

    let mut config = load_config(&store).await?;

    // Prometheus metrics.
    let _exporter = PrometheusExporter::start(
        cli_options.prometheus_port,
        prometheus::default_registry().clone(),
    )?;

    info!("Initializing bisect request handler");
    let (tx_indexers, rx_indexers) = watch::channel(vec![]);
    {
        let store_clone = store.clone();

        let ctx = GraphixState::new(store_clone.clone(), config_receiver.clone());

        let networks: Vec<models::NewNetwork> = config
            .chains
            .iter()
            .map(|(name, config)| models::NewNetwork {
                name: name.clone(),
                caip2: config.caip2.clone(),
            })
            .collect();
        store_clone.create_networks_if_missing(&networks).await?;

        tokio::spawn(async move {
            handle_divergence_investigation_requests(&store_clone, rx_indexers, &ctx)
                .await
                .unwrap()
        });
    }

    loop {
        config = load_config(&store).await?;
        config_sender.send(config.clone()).ok();

        let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

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
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::from_str(
                &env::var("RUST_LOG").unwrap_or_else(|_| "graphix=debug".to_string()),
            )
            .unwrap(),
        )
        .init();
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
