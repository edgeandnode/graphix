mod bisect;

#[cfg(test)]
mod tests;

use clap::Parser;
use graphix_common::prelude::{BlockPointer, Config, Indexer, ProofOfIndexing, SubgraphDeployment};
use graphix_common::queries::{query_indexing_statuses, query_proofs_of_indexing};
use graphix_common::PrometheusExporter;
use graphix_common::{config, store};
use prometheus_exporter::prometheus;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::*;
use tracing_subscriber;

use crate::bisect::PoiBisectingContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    info!("Parse options");
    let cli_options = CliOptions::parse();

    info!("Load configuration file");
    let config = Config::read(&cli_options.config)?;

    let store = store::Store::new(&config.database_url).await?;

    let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

    // Prometheus metrics.
    let registry = prometheus::default_registry().clone();
    let _exporter = PrometheusExporter::start(9184, registry.clone()).unwrap();

    let store_clone = store.clone();
    let (tx_indexers, rx_indexers) = watch::channel(vec![]);
    tokio::spawn(async move {
        handle_bisect_requests(&store_clone, rx_indexers)
            .await
            .unwrap()
    });

    loop {
        info!("New main loop iteration");

        info!("Initialize inputs (indexers, indexing statuses etc.)");
        let indexers = config::config_to_indexers(config.clone()).await?;
        tx_indexers.send(indexers.clone())?;

        let indexing_statuses = query_indexing_statuses(indexers).await;

        info!("Monitor proofs of indexing");
        let pois = query_proofs_of_indexing(indexing_statuses).await;

        let write_err = store.write_pois(&pois, store::PoiLiveness::Live).err();
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

async fn handle_bisect_requests(
    store: &store::Store,
    indexers: watch::Receiver<Vec<Arc<dyn Indexer>>>,
) -> anyhow::Result<()> {
    loop {
        let next_request = store.recv_cross_check_report_request().await?;
        let poi_block = store
            .poi(&next_request.req.poi1)?
            .expect("POI not found")
            .block
            .number;
        let poi1 = store.poi(&next_request.req.poi1)?.expect("POI not found");
        let poi2 = store.poi(&next_request.req.poi2)?.expect("POI not found");
        let indexer1 = indexers
            .borrow()
            .iter()
            .find(|indexer| indexer.address() == poi1.indexer.address.as_deref())
            .expect("Indexer not found")
            .clone();
        let indexer2 = indexers
            .borrow()
            .iter()
            .find(|indexer| indexer.address() == poi2.indexer.address.as_deref())
            .expect("Indexer not found")
            .clone();
        let deployment = SubgraphDeployment(poi1.sg_deployment.cid);
        let block = BlockPointer {
            number: poi_block as _,
            hash: None,
        };
        let poi1 = ProofOfIndexing {
            indexer: indexer1.clone(),
            deployment: deployment.clone(),
            block,
            proof_of_indexing: poi1.poi.try_into()?,
        };
        let poi2 = ProofOfIndexing {
            indexer: indexer2.clone(),
            deployment: deployment.clone(),
            block,
            proof_of_indexing: poi2.poi.try_into()?,
        };
        let context = PoiBisectingContext::new(
            next_request.uuid.to_string(),
            poi1,
            poi2,
            deployment.clone(),
        );

        let bisect_result = context.start().await?;

        println!("Bisect result: {:?}", bisect_result,);
    }
}

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}
