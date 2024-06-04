use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use graphix_common_types::{
    BisectionReport, BisectionRunReport, DivergenceBlockBounds, DivergenceInvestigationReport,
    DivergenceInvestigationStatus, DivergingBlock as DivergentBlock, HexString, PartialBlock,
    PoiBytes,
};
use graphix_indexer_client::{
    IndexerClient, IndexerId, PoiRequest, ProofOfIndexing, SubgraphDeployment,
};
use graphix_store::models::DivergenceInvestigationRequest;
use graphix_store::Store;
use thiserror::Error;
use tokio::sync::watch;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::graphql_api::api_types::{self, Indexer};
use crate::graphql_api::ServerState;

pub struct DivergingBlock {
    pub poi1: ProofOfIndexing,
    pub poi2: ProofOfIndexing,
}

impl From<DivergingBlock> for DivergentBlock {
    fn from(other: DivergingBlock) -> DivergentBlock {
        Self {
            block: PartialBlock {
                number: other.poi1.block.number as _,
                hash: other.poi1.block.hash.map(|h| HexString(h.0.to_vec())),
            },
            proof_of_indexing1: other.poi1.proof_of_indexing,
            proof_of_indexing2: other.poi2.proof_of_indexing,
        }
    }
}

pub struct PoiBisectingContext {
    report: BisectionRunReport,
    bisection_id: Uuid,
    poi1_data: PoiWithRelatedData,
    poi2_data: PoiWithRelatedData,
}

impl PoiBisectingContext {
    fn new(
        report: BisectionRunReport,
        bisection_id: Uuid,
        poi1_data: PoiWithRelatedData,
        poi2_data: PoiWithRelatedData,
    ) -> anyhow::Result<Self> {
        // Before attempting to bisect Pois, we need to make sure that the Pois refer to:
        // 1. the same subgraph deployment, and
        // 2. the same block.

        anyhow::ensure!(poi1_data.deployment.cid() == poi2_data.deployment.cid());
        anyhow::ensure!(poi1_data.block.number() == poi2_data.block.number());
        // FIXME!
        // Let's also check block hashes are present (and identical, by extension).
        //anyhow::ensure!(poi1.block.hash.is_some());
        //anyhow::ensure!(poi2.block.hash.is_some());
        //anyhow::ensure!(poi1.proof_of_indexing != poi2.proof_of_indexing);
        //anyhow::ensure!(poi1.indexer.address() != poi2.indexer.address());

        Ok(Self {
            report,
            bisection_id,
            poi1_data,
            poi2_data,
        })
    }

    fn deployment(&self) -> &api_types::SubgraphDeployment {
        &self.poi1_data.deployment
    }

    pub async fn start(mut self) -> (BisectionRunReport, u64) {
        let deployment: api_types::SubgraphDeployment = self.deployment().clone();

        let indexer1 = self.poi1_data.indexer_client.clone();
        let indexer2 = self.poi2_data.indexer_client.clone();

        info!(
            bisection_id = %self.bisection_id,
            deployment = ?deployment.cid(),
            "Starting Poi bisecting"
        );

        // The range of block numbers that we're investigating is bounded
        // inclusively both below and above. The bisection algorithm will
        // continue searching until only a single block number is left in the
        // range.
        let mut bounds = 0..=self.poi1_data.block.number();

        loop {
            let block_number = (bounds.start() + bounds.end()) / 2;

            debug!(
                bisection_id = %self.bisection_id,
                deployment = ?deployment.cid(),
                lower_bound = ?bounds.start(),
                upper_bound = ?bounds.end(),
                block_number,
                "Bisecting Pois"
            );

            let poi1 = indexer1
                .clone()
                .proof_of_indexing(PoiRequest {
                    deployment: SubgraphDeployment(deployment.cid().to_string()),
                    block_number,
                })
                .await;
            let poi2 = indexer2
                .clone()
                .proof_of_indexing(PoiRequest {
                    deployment: SubgraphDeployment(deployment.cid().to_string()),
                    block_number,
                })
                .await;

            let bisect = BisectionReport {
                block: PartialBlock {
                    number: block_number as _,
                    hash: None,
                },
                indexer1_response: format!("{:?}", poi1),
                indexer2_response: format!("{:?}", poi2),
            };
            self.report.bisects.push(bisect);

            if poi1.ok() == poi2.ok() {
                bounds = block_number..=*bounds.end();
                self.report.divergence_block_bounds.lower_bound.number = block_number as _;
            } else {
                bounds = *bounds.start()..=block_number;
                self.report.divergence_block_bounds.upper_bound.number = block_number as _;
            }

            if bounds.start() == bounds.end() {
                break;
            }
        }

        let diverging_block = *bounds.start();
        (self.report, diverging_block)
    }
}

#[derive(Debug, Error)]
pub enum DivergenceInvestigationError {
    #[error("Too many POIs in a single request, the max. is {max}")]
    TooManyPois { max: u32 },
    #[error("No indexer(s) that produced the given Poi were found in the Graphix database")]
    IndexerNotFound { poi: String },
    #[error(
        "The two Pois were produced by the same indexer ({indexer_id}), bisecting the difference is not possible"
    )]
    SameIndexer { indexer_id: String },
    #[error("The two Pois were produced for different deployments, they cannot be compared: {poi1}: {poi1_deployment}, {poi2}: {poi2_deployment}")]
    DifferentDeployments {
        poi1: String,
        poi2: String,
        poi1_deployment: String,
        poi2_deployment: String,
    },
    #[error("The two Pois were produced for different blocks, they cannot be compared: {poi1}: {poi1_block}, {poi2}: {poi2_block}")]
    DifferentBlocks {
        poi1: String,
        poi2: String,
        poi1_block: i64,
        poi2_block: i64,
    },
    #[error(transparent)]
    Database(anyhow::Error),
}

pub async fn handle_divergence_investigation_requests(
    store: &Store,
    indexers: watch::Receiver<Vec<Arc<dyn IndexerClient>>>,
    ctx: &ServerState,
) -> anyhow::Result<()> {
    loop {
        debug!("Checking for new divergence investigation requests");

        let (req_uuid, req_contents_blob) = {
            loop {
                let req_opt = store
                    .get_first_pending_divergence_investigation_request()
                    .await?;
                if let Some(req) = req_opt {
                    break req;
                } else {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            }
        };
        debug!(?req_uuid, "Found new divergence investigation request");

        let req_contents =
            serde_json::from_value(req_contents_blob).expect("invalid request blob; this is a bug");
        let report = handle_divergence_investigation_request(
            store,
            &req_uuid,
            req_contents,
            indexers.clone(),
            ctx,
        )
        .await;

        let serialized_report = serde_json::to_value(&report).unwrap();
        debug!(
            ?req_uuid,
            "Writing divergence investigation report to database"
        );
        store
            .create_or_update_divergence_investigation_report(&req_uuid, serialized_report)
            .await?;
        store
            .delete_divergence_investigation_request(&req_uuid)
            .await?;
    }
}

/// Just a group of data related to a PoI, that is needed to perform a
/// bisection.
struct PoiWithRelatedData {
    poi: api_types::ProofOfIndexing,
    deployment: api_types::SubgraphDeployment,
    block: api_types::Block,
    indexer: Indexer,
    indexer_client: Arc<dyn IndexerClient>,
}

impl PoiWithRelatedData {
    async fn new(
        poi_bytes: &PoiBytes,
        store: &Store,
        indexers: &[Arc<dyn IndexerClient>],
        ctx: &ServerState,
    ) -> anyhow::Result<Option<Self>> {
        let Some(poi_model) = store.poi(poi_bytes).await? else {
            return Ok(None);
        };

        let poi = api_types::ProofOfIndexing { model: poi_model };

        let deployment = poi
            .deployment(ctx)
            .await
            .map_err(|err| anyhow!("failed to load deployment: {err}"))?;

        let block = poi
            .block(ctx)
            .await
            .map_err(|err| anyhow!("failed to load block: {err}"))?;

        let indexer = poi
            .indexer(ctx)
            .await
            .map_err(|err| anyhow!("failed to load indexer: {err}"))?;

        let indexer_client = indexers
            .iter()
            .find(|indexer| indexer.address() == indexer.address())
            .cloned()
            .ok_or_else(|| anyhow!("indexer not found"))?;

        Ok(Some(Self {
            poi,
            deployment,
            block,
            indexer,
            indexer_client,
        }))
    }
}

async fn handle_divergence_investigation_request_pair(
    store: &Store,
    indexers: &[Arc<dyn IndexerClient>],
    req_uuid: &Uuid,
    poi1_s: &PoiBytes,
    poi2_s: &PoiBytes,
    ctx: &ServerState,
) -> BisectionRunReport {
    debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Bisecting Pois");

    let mut report = BisectionRunReport {
        bisects: vec![],
        uuid: Uuid::new_v4(),
        poi1: *poi1_s,
        poi2: *poi2_s,
        divergence_block_bounds: DivergenceBlockBounds {
            lower_bound: PartialBlock {
                number: 1,
                hash: None,
            },
            upper_bound: PartialBlock {
                number: 1 as _,
                hash: None,
            },
        },
        error: None,
    };

    debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Fetching Pois");
    let poi1_data = match PoiWithRelatedData::new(poi1_s, store, indexers, ctx).await {
        Ok(Some(data)) => data,
        Ok(None) => return report,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    let poi2_data = match PoiWithRelatedData::new(poi2_s, store, indexers, ctx).await {
        Ok(Some(data)) => data,
        Ok(None) => return report,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };

    debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Fetched Pois");

    report.divergence_block_bounds.upper_bound.number = poi1_data.block.number_i64();

    // Two PoIs need to relate to the same subgraph deployment to be comparable.
    if poi1_data.deployment.cid() != poi2_data.deployment.cid() {
        report.error = Some(
            DivergenceInvestigationError::DifferentDeployments {
                poi1: poi1_s.to_string(),
                poi2: poi2_s.to_string(),
                poi1_deployment: poi1_data.deployment.cid().to_string(),
                poi2_deployment: poi2_data.deployment.cid().to_string(),
            }
            .to_string(),
        );
    }

    // Two PoIs need to have the same block number to be comparable.
    if poi1_data.block.number() != poi2_data.block.number() {
        report.error = Some(
            DivergenceInvestigationError::DifferentBlocks {
                poi1: poi1_s.to_string(),
                poi2: poi2_s.to_string(),
                poi1_block: poi1_data.block.number_i64(),
                poi2_block: poi2_data.block.number_i64(),
            }
            .to_string(),
        );
    }

    debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Fetching indexers");

    debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Fetched indexers");
    if poi1_data.indexer.address() == poi2_data.indexer.address() {
        let indexer_id = poi1_data.indexer.address().to_string();
        report.error = Some(DivergenceInvestigationError::SameIndexer { indexer_id }.to_string());
        return report;
    }

    let bisection_uuid = Uuid::new_v4();

    let context = PoiBisectingContext::new(report, bisection_uuid, poi1_data, poi2_data)
        .expect("bisect context creation failed");
    let (report, _block_num) = context.start().await;

    report
}

async fn handle_divergence_investigation_request(
    store: &Store,
    req_uuid: &Uuid,
    req_contents: DivergenceInvestigationRequest,
    indexers: watch::Receiver<Vec<Arc<dyn IndexerClient>>>,
    ctx: &ServerState,
) -> DivergenceInvestigationReport {
    let mut report = DivergenceInvestigationReport {
        uuid: req_uuid.clone(),
        status: DivergenceInvestigationStatus::Complete,
        bisection_runs: vec![],
        error: None,
    };

    // The number of bisections is quadratic to the number of Pois, so it's
    // important not to allow too many in a single request.
    const MAX_NUMBER_OF_POIS_PER_REQUEST: u32 = 4;

    if req_contents.pois.len() > MAX_NUMBER_OF_POIS_PER_REQUEST as usize {
        report.error = Some(
            DivergenceInvestigationError::TooManyPois {
                max: MAX_NUMBER_OF_POIS_PER_REQUEST,
            }
            .to_string(),
        );
        return report;
    }

    let indexers = indexers.borrow().clone();

    let poi_pairs = unordered_pairs_combinations(req_contents.pois.into_iter());

    for (poi1_s, poi2_s) in poi_pairs.into_iter() {
        let bisection_run_report = handle_divergence_investigation_request_pair(
            store, &indexers, req_uuid, &poi1_s, &poi2_s, ctx,
        )
        .await;
        debug!(?req_uuid, poi1 = %poi1_s, poi2 = %poi2_s, "Finished bisection run");
        report.bisection_runs.push(bisection_run_report);
        let report_json = serde_json::to_value(&report).unwrap();
        if let Err(err) = store
            .create_or_update_divergence_investigation_report(req_uuid, report_json)
            .await
        {
            error!(?req_uuid, error = %err, "Failed to upsert divergence investigation report to the database");
        }
    }

    info!(?req_uuid, "Finished bisecting Pois");

    report
}

/// Creates all combinations of elements in the iterator, without duplicates.
/// Elements are never paired with themselves.
pub fn unordered_pairs_combinations<T>(iter: impl Iterator<Item = T> + Clone) -> HashSet<(T, T)>
where
    T: Hash + Eq + Clone,
{
    let mut pairs = HashSet::new();
    for (i, x) in iter.clone().enumerate() {
        for y in iter.clone().skip(i + 1) {
            pairs.insert((x.clone(), y));
        }
    }
    pairs
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn test_unordered_pairs_combinations(original: Vec<u32>, combinations: Vec<(u32, u32)>) {
        assert_eq!(
            unordered_pairs_combinations(original.into_iter()),
            HashSet::from_iter(combinations.into_iter())
        );
    }

    #[test]
    fn unordered_pairs_combinations_test_cases() {
        test_unordered_pairs_combinations(vec![], vec![]);
        test_unordered_pairs_combinations(vec![1], vec![]);
        test_unordered_pairs_combinations(vec![1, 2], vec![(1, 2)]);
        test_unordered_pairs_combinations(vec![1, 2, 3], vec![(1, 2), (2, 3), (1, 3)]);
    }
}
