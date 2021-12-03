mod config;
mod indexer;
mod indexing_statuses;
mod modes;
mod pois;
pub mod types;

use std::path::PathBuf;
use structopt::StructOpt;
use tokio;
use tracing::*;
use tracing_subscriber::{self, layer::SubscriberExt as _, util::SubscriberInitExt as _};

use config::Config;

#[derive(StructOpt, Debug)]
struct Options {
    #[structopt(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or(tracing_subscriber::EnvFilter::try_new("info,graph_ixi=debug").unwrap());
    let defaults = tracing_subscriber::registry().with(filter_layer);
    let fmt_layer = tracing_subscriber::fmt::layer();
    defaults.with(fmt_layer).init();

    info!("Parse options");
    let options = Options::from_args();

    info!("Load configuration file");
    let config = Config::try_from(&options.config)?;

    info!("Initialize inputs (indexers, indexing statuses etc.)");
    let indexers = match config {
        Config::Testing(testing) => modes::testing_indexers(testing.clone()),
        _ => todo!(),
    };

    info!("Monitor indexing statuses");
    let indexers_with_statuses = indexing_statuses::indexing_statuses(indexers);

    info!("Monitor proofs of indexing");
    let pois = pois::proofs_of_indexing(indexers_with_statuses);

    // Temporary loop to keep things running and print the latest results
    let mut pois = pois.subscribe();
    loop {
        let pois = pois.next().await.unwrap();
        dbg!(pois);
    }

    Ok(())
}
