pub(crate) mod bisect;
pub(crate) mod cross_checking;

#[cfg(test)]
mod tests;

use clap::Parser;
use graphix_common::{db, modes, prelude::Config};
use graphix_common::{indexing_statuses, proofs_of_indexing};
use std::path::PathBuf;
use tracing::*;
use tracing_subscriber::{self, layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::try_new(
            "info,graphix_common=debug,graphix_cross_checker=debug",
        )
        .unwrap()
    });
    let defaults = tracing_subscriber::registry().with(filter_layer);
    let fmt_layer = tracing_subscriber::fmt::layer();
    defaults.with(fmt_layer).init();

    info!("Parse options");
    let options = CliOptions::parse();

    info!("Load configuration file");
    let config = Config::try_from(&options.config)?;

    let db_url = match &config {
        Config::Testing(testing) => testing.database_url.as_str(),
        _ => todo!(),
    };
    let store = db::Store::new(db_url)?;

    info!("Initialize inputs (indexers, indexing statuses etc.)");
    let indexers = match config {
        Config::Testing(testing) => modes::testing_indexers(testing.clone()),
        _ => todo!("Only testing mode supported for now"),
    };

    loop {
        info!("Monitor indexing statuses");
        let indexing_statuses = indexing_statuses::query_indexing_statuses(indexers.clone()).await;

        info!("Monitor proofs of indexing");
        let pois = proofs_of_indexing::query_proofs_of_indexing(indexing_statuses).await;

        store.write_pois(&pois)?;

        // Reports are a stream that should be written to the database
        //db::proofs_of_indexing::write_reports(store, reports);

        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
    }
}
