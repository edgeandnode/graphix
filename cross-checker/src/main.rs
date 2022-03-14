pub mod indexing_statuses;
pub mod proofs_of_indexing;
mod server;

#[cfg(test)]
mod tests;

extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

use diesel::{r2d2, PgConnection};
use graph_ixi_common::{db, modes, prelude::Config};
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;
use tokio;
use tracing::*;
use tracing_subscriber::{self, layer::SubscriberExt as _, util::SubscriberInitExt as _};

embed_migrations!("../migrations");

#[derive(StructOpt, Debug)]
struct Options {
    #[structopt(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or(
        tracing_subscriber::EnvFilter::try_new(
            "info,graph_ixi_common=debug,graph_ixi_cross_checker=debug",
        )
        .unwrap(),
    );
    let defaults = tracing_subscriber::registry().with(filter_layer);
    let fmt_layer = tracing_subscriber::fmt::layer();
    defaults.with(fmt_layer).init();

    info!("Parse options");
    let options = Options::from_args();

    info!("Load configuration file");
    let config = Config::try_from(&options.config)?;

    info!("Connect to database");
    let db_url = match &config {
        Config::Testing(testing) => testing.database_url.as_str(),
        _ => todo!(),
    };
    let db_connection_manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
    let db_connection_pool = Arc::new(r2d2::Builder::new().build(db_connection_manager)?);

    info!("Run database migrations");
    let connection = db_connection_pool.get()?;
    embedded_migrations::run(&connection)?;

    info!("Initialize inputs (indexers, indexing statuses etc.)");
    let indexers = match config {
        Config::Testing(testing) => modes::testing_indexers(testing.clone()),
        _ => todo!(),
    };

    info!("Monitor indexing statuses");
    let indexing_statuses = indexing_statuses::indexing_statuses(indexers);

    info!("Monitor proofs of indexing");
    let pois = proofs_of_indexing::proofs_of_indexing(indexing_statuses);

    info!("Start POI cross checking");
    let (pois, reports) = proofs_of_indexing::cross_checking(pois);

    // POIs are a stream that should be written to the POI database
    db::proofs_of_indexing::write(db_connection_pool.clone(), pois);

    // Reports are a stream that should be written to the database
    db::proofs_of_indexing::write_reports(db_connection_pool, reports);

    // Power up the web server
    server::run().await
}