pub(crate) mod cross_checking;

#[cfg(test)]
mod tests;

use clap::Parser;
use graphix_common::{config, db, prelude::Config};
use graphix_common::{indexing_statuses, proofs_of_indexing, PrometheusExporter};
use prometheus_exporter::prometheus;
use std::path::PathBuf;
use std::time::Duration;
use tracing::*;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    info!("Parse options");
    let cli_options = CliOptions::parse();

    info!("Load configuration file");
    let config = Config::read(&cli_options.config)?;

    let store = db::Store::new(&config.database_url)?;

    let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

    // Prometheus metrics.
    let registry = prometheus::default_registry().clone();
    let _exporter = PrometheusExporter::start(9184, registry.clone()).unwrap();

    loop {
        info!("New main loop iteration");

        info!("Initialize inputs (indexers, indexing statuses etc.)");
        let indexers = config::config_to_indexers(config.clone()).await?;

        let indexing_statuses = indexing_statuses::query_indexing_statuses(indexers).await;

        info!("Monitor proofs of indexing");
        let pois = proofs_of_indexing::query_proofs_of_indexing(indexing_statuses).await;

        let write_err = store.write_pois(&pois, db::PoiLiveness::Live).err();
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

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}
