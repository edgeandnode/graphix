mod bisect;
mod utils;

use clap::Parser;
use graphix_common::api_types::DivergenceInvestigationRequest;
use graphix_common::prelude::{BlockPointer, Config, Indexer, ProofOfIndexing, SubgraphDeployment};
use graphix_common::queries::{query_indexing_statuses, query_proofs_of_indexing};
use graphix_common::{config, store};
use graphix_common::{metrics, PrometheusExporter};
use prometheus_exporter::prometheus;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::watch;
use tracing::*;
use tracing_subscriber;
use utils::unordered_pairs_combinations;
use uuid::Uuid;

use crate::bisect::PoiBisectingContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    info!("Parse options");
    let cli_options = CliOptions::parse();

    info!("Loading configuration file");
    let config = Config::read(&cli_options.config)?;

    info!("Initialize store and running migrations");
    let store = store::Store::new(&config.database_url).await?;
    info!("Store initialization successful");

    let sleep_duration = Duration::from_secs(config.polling_period_in_seconds);

    // Prometheus metrics.
    let registry = prometheus::default_registry().clone();
    let _exporter = PrometheusExporter::start(config.prometheus_port, registry.clone()).unwrap();

    info!("Initializing bisect request handler");
    let store_clone = store.clone();
    let (tx_indexers, rx_indexers) = watch::channel(vec![]);
    tokio::spawn(async move {
        handle_new_divergence_investigation_requests(&store_clone, rx_indexers)
            .await
            .unwrap()
    });

    loop {
        info!("New main loop iteration");
        info!("Initialize inputs (indexers, indexing statuses etc.)");

        let mut indexers = config::config_to_indexers(config.clone()).await?;
        // Different data sources, especially network subgraphs, result in
        // duplicate indexers.
        indexers = deduplicate_indexers(&indexers);

        tx_indexers.send(indexers.clone())?;

        let indexing_statuses = query_indexing_statuses(indexers, metrics()).await;

        info!("Monitor proofs of indexing");
        let pois = query_proofs_of_indexing(indexing_statuses, config.block_choice_policy).await;

        info!(pois = pois.len(), "Finished tracking PoIs");

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

fn deduplicate_indexers(indexers: &[Arc<dyn Indexer>]) -> Vec<Arc<dyn Indexer>> {
    info!(len = indexers.len(), "Deduplicating indexers");
    let mut seen = HashSet::new();
    let mut deduplicated = vec![];
    for indexer in indexers {
        if !seen.contains(indexer.id()) {
            deduplicated.push(indexer.clone());
            seen.insert(indexer.id().to_string());
        }
    }
    info!(
        len = deduplicated.len(),
        delta = indexers.len() - deduplicated.len(),
        "Successfully deduplicated indexers"
    );
    deduplicated
}

#[derive(Parser, Debug)]
struct CliOptions {
    #[clap(long)]
    config: PathBuf,
}

#[derive(Debug, Error)]
pub enum DivergenceInvestigationError {
    #[error("Too many POIs in a single request, the max. is {max}")]
    TooManyPois { max: u32 },
    #[error("No indexer(s) that produced the given PoI were found in the Graphix database")]
    IndexerNotFound { poi: String },
    #[error(
        "The two PoIs were produced by the same indexer ({indexer_id}), bisecting the difference is not possible"
    )]
    SameIndexer { indexer_id: String },
    #[error("The two PoIs were produced for different deployments, they cannot be compared: {poi1}: {poi1_deployment}, {poi2}: {poi2_deployment}")]
    DifferentDeployments {
        poi1: String,
        poi2: String,
        poi1_deployment: String,
        poi2_deployment: String,
    },
    #[error("The two PoIs were produced for different blocks, they cannot be compared: {poi1}: {poi1_block}, {poi2}: {poi2_block}")]
    DifferentBlocks {
        poi1: String,
        poi2: String,
        poi1_block: i64,
        poi2_block: i64,
    },
    #[error(transparent)]
    Database(anyhow::Error),
}

async fn handle_new_divergence_investigation_requests(
    store: &store::Store,
    indexers: watch::Receiver<Vec<Arc<dyn Indexer>>>,
) -> anyhow::Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        debug!("Checking for new divergence investigation requests");
        let (req_uuid, req_contents) =
            if let Some(x) = store.get_first_divergence_investigation_request()? {
                x
            } else {
                continue;
            };
        info!(req_uuid, "Found new divergence investigation request");
        let res = handle_new_divergence_investigation_request(
            store,
            &req_uuid,
            req_contents,
            indexers.clone(),
        )
        .await;
        if let Err(err) = res {
            error!(error = %err, "Failed to handle bisect request");
        }
        store.delete_divergence_investigation_request(&req_uuid)?;
    }
}

async fn handle_new_divergence_investigation_request_pair(
    store: &store::Store,
    indexers: &[Arc<dyn Indexer>],
    req_uuid_str: &str,
    poi1_s: &str,
    poi2_s: &str,
) -> Result<(), DivergenceInvestigationError> {
    debug!(req_uuid = req_uuid_str, poi1 = %poi1_s, poi2 = %poi2_s, "Bisecting PoIs");

    let poi1 = store
        .poi(&poi1_s)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|poi_opt| {
            if let Some(poi) = poi_opt {
                Ok(poi)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi1_s.to_string(),
                })
            }
        })?;
    let poi2 = store
        .poi(&poi2_s)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|poi_opt| {
            if let Some(poi) = poi_opt {
                Ok(poi)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi2_s.to_string(),
                })
            }
        })?;

    if poi1.sg_deployment.cid != poi2.sg_deployment.cid {
        return Err(DivergenceInvestigationError::DifferentDeployments {
            poi1: poi1_s.to_string(),
            poi2: poi2_s.to_string(),
            poi1_deployment: poi1.sg_deployment.cid.to_string(),
            poi2_deployment: poi2.sg_deployment.cid.to_string(),
        }
        .into());
    }

    if poi1.block.number != poi2.block.number {
        return Err(DivergenceInvestigationError::DifferentBlocks {
            poi1: poi1_s.to_string(),
            poi2: poi2_s.to_string(),
            poi1_block: poi1.block.number,
            poi2_block: poi2.block.number,
        }
        .into());
    }

    let deployment = SubgraphDeployment(poi1.sg_deployment.cid);
    let block = BlockPointer {
        number: poi1.block.number as _,
        hash: None,
    };

    let indexer1 = indexers
        .iter()
        .find(|indexer| indexer.address() == poi1.indexer.address.as_deref())
        .cloned()
        .ok_or(DivergenceInvestigationError::IndexerNotFound {
            poi: poi1_s.to_string(),
        })?;
    let indexer2 = indexers
        .iter()
        .find(|indexer| indexer.address() == poi2.indexer.address.as_deref())
        .cloned()
        .ok_or(DivergenceInvestigationError::IndexerNotFound {
            poi: poi2_s.to_string(),
        })?;

    if indexer1.id() == indexer2.id() {
        return Err(DivergenceInvestigationError::SameIndexer {
            indexer_id: indexer1.id().to_string(),
        }
        .into());
    }

    let bisection_uuid = Uuid::new_v4().to_string();

    let poi1 = ProofOfIndexing {
        indexer: indexer1.clone(),
        deployment: deployment.clone(),
        block,
        proof_of_indexing: poi1.poi.try_into().expect("poi1 conversion failed"),
    };
    let poi2 = ProofOfIndexing {
        indexer: indexer2.clone(),
        deployment: deployment.clone(),
        block,
        proof_of_indexing: poi2.poi.try_into().expect("poi2 conversion failed"),
    };

    let context = PoiBisectingContext::new(bisection_uuid, poi1, poi2, deployment.clone())
        .expect("bisect context creation failed");
    context.start().await.expect("bisect failed");

    Ok(())
}

async fn handle_new_divergence_investigation_request(
    store: &store::Store,
    req_uuid_str: &str,
    req_contents: DivergenceInvestigationRequest,
    indexers: watch::Receiver<Vec<Arc<dyn Indexer>>>,
) -> Result<(), DivergenceInvestigationError> {
    // The number of bisections is quadratic to the number of PoIs, so it's
    // important not to allow too many in a single request.
    const MAX_NUMBER_OF_POIS_PER_REQUEST: u32 = 4;

    if req_contents.pois.len() > MAX_NUMBER_OF_POIS_PER_REQUEST as usize {
        return Err(DivergenceInvestigationError::TooManyPois {
            max: MAX_NUMBER_OF_POIS_PER_REQUEST,
        }
        .into());
    }

    let indexers = indexers.borrow().clone();

    let poi_pairs = unordered_pairs_combinations(req_contents.pois.into_iter());

    for (poi1_s, poi2_s) in poi_pairs.into_iter() {
        if let Err(e) = handle_new_divergence_investigation_request_pair(
            store,
            &indexers,
            req_uuid_str,
            &poi1_s,
            &poi2_s,
        )
        .await
        {
            error!(req_uuid_str, error = %e, "Failed to bisect PoIs");
        }
    }

    info!(req_uuid = req_uuid_str, "Finished bisecting PoIs");

    Ok(())
}
