pub(crate) mod bisect;
pub(crate) mod cross_checking;

#[cfg(test)]
mod tests;

use clap::Parser;
use graphix_common::{db, modes, prelude::Config};
use graphix_common::{indexing_statuses, proofs_of_indexing};
use std::path::PathBuf;
use tracing::*;
use tracing_subscriber::{self};

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    info!("Parse options");
    let options = CliOptions::parse();

    info!("Load configuration file");
    let config = match Config::try_from(&options.config)? {
        Config::Testing(c) => c,
        _ => todo!("Only testing mode supported for now"),
    };

    let store = db::Store::new(config.database_url.as_str())?;

    info!("Initialize inputs (indexers, indexing statuses etc.)");
    let indexers = modes::testing_indexers(config.clone());

    loop {
        info!("Monitor indexing statuses");
        let indexing_statuses = indexing_statuses::query_indexing_statuses(indexers.clone()).await;

        info!("Monitor proofs of indexing");
        let pois = proofs_of_indexing::query_proofs_of_indexing(indexing_statuses).await;

        store.write_pois(&pois)?;

        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
    }
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}
