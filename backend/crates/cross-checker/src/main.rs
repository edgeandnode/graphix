pub(crate) mod bisect;
pub(crate) mod cross_checking;
mod server;

#[cfg(test)]
mod tests;

use clap::Parser;
use eventuals::EventualExt;
use futures::channel::mpsc::channel;
use futures::SinkExt;
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
        _ => todo!(),
    };

    info!("Monitor indexing statuses");
    let indexing_statuses = indexing_statuses::indexing_statuses(indexers);

    info!("Monitor proofs of indexing");
    let pois = proofs_of_indexing::proofs_of_indexing(indexing_statuses);

    //info!("Start POI cross checking");
    //let (pois, reports) = proofs_of_indexing::cross_checking(pois);

    let (poi_broadcaster, poi_receiver) = channel(1000);

    let pipe = pois.pipe_async(move |pois| {
        let mut poi_broadcaster = poi_broadcaster.clone();

        async move {
            for poi in pois {
                poi_broadcaster.send(poi).await.unwrap();
            }
        }
    });

    pipe.forever();

    // POIs are a stream that should be written to the POI database
    db::proofs_of_indexing::write(store.clone(), poi_receiver);

    // Reports are a stream that should be written to the database
    //db::proofs_of_indexing::write_reports(store, reports);

    // Power up the web server
    server::run().await
}
