pub(crate) mod cross_checking;

#[cfg(test)]
mod tests;

use clap::Parser;
use graphix_common::{db, modes, prelude::Config};
use graphix_common::{indexing_statuses, proofs_of_indexing};
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
    let config = match Config::try_from(&cli_options.config)? {
        Config::Testing(c) => c,
        _ => todo!("Only testing mode supported for now"),
    };

    let store = db::Store::new(&config.database_url)?;

    info!("Initialize inputs (indexers, indexing statuses etc.)");
    let indexers = modes::testing_indexers(config.clone());

    let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

    loop {
        info!("Monitor indexing statuses");
        let indexing_statuses = indexing_statuses::query_indexing_statuses(indexers.clone()).await;

        info!("Monitor proofs of indexing");
        let pois = proofs_of_indexing::query_proofs_of_indexing(indexing_statuses).await;

        store.write_pois(&pois, db::PoiLiveness::Live)?;

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
