use std::{collections::BTreeSet, sync::Arc};

use anyhow::anyhow;
use eventuals::{Eventual, EventualExt};
use futures::{
    channel::mpsc::{channel, Sender},
    stream::FuturesUnordered,
    FutureExt, SinkExt, Stream, StreamExt,
};
use graph_ixi_common::prelude::{
    Indexer, POICrossCheckReport, POIRequest, ProofOfIndexing, SubgraphDeployment,
};
use itertools::Itertools;
use nanoid::nanoid;
use tracing::{debug, info, warn};

use crate::proofs_of_indexing::DivergingBlock;

use super::{bisect_blocks, BisectDecision};

#[derive(Debug, Clone)]
struct POIBisectContext<I>
where
    I: Indexer,
{
    indexer1: Arc<I>,
    indexer2: Arc<I>,
    deployment: SubgraphDeployment,
    poi_broadcaster: Sender<ProofOfIndexing<I>>,
}

pub fn cross_checking<I>(
    pois: Eventual<Vec<ProofOfIndexing<I>>>,
) -> (
    impl Stream<Item = ProofOfIndexing<I>>,
    impl Stream<Item = POICrossCheckReport<I>>,
)
where
    I: Indexer + 'static,
{
    let (poi_broadcaster, poi_receiver) = channel(1000);
    let (report_broadcaster, report_receiver) = channel(1000);

    let pipe =
        pois.pipe_async(move |mut pois| {
            let poi_broadcaster = poi_broadcaster.clone();
            let report_broadcaster = report_broadcaster.clone();

            async move {
                // Sort POIs (to make everything a little more predictable)
                pois.sort();

                // Build a flat, unique list of all deployments we have POIs for
                let deployments = pois
                    .iter()
                    .map(|poi| &poi.deployment)
                    .collect::<BTreeSet<_>>();

                // Build a map of deployments to Indexers/POIs
                let reports = deployments
                    .into_iter()
                    .map(|deployment| {
                        (
                            deployment,
                            pois.iter()
                                .filter(|poi| poi.deployment.eq(deployment))
                                .map(|poi| poi.to_owned())
                                .collect::<Vec<_>>(),
                        )
                    })
                    .flat_map(|(deployment, pois)| {
                        info!(
                            deployment = %deployment.as_str(),
                            "Cross-checking POIs for deployment"
                        );

                        // Get all pairs of POIS/indexers to compare against each other
                        let count = pois.len();
                        let combinations = pois
                            .into_iter()
                            .tuple_combinations::<(_, _)>()
                            .collect_vec();

                        if count > 0 && combinations.len() == 0 {
                            warn!(
                                indexers = %count,
                                deployment = %deployment.as_str(),
                                "Deployment has POIs but not enough indexers to cross-check",
                            );
                            return vec![];
                        }
                        combinations
                    })
                    .map(|(poi1, poi2)| cross_check_poi(poi1, poi2, poi_broadcaster.clone()))
                    .collect::<FuturesUnordered<_>>();

                reports
                    .forward(report_broadcaster.clone().sink_map_err(|e| {
                        anyhow!("Failed to broadcast POI cross-check report: {}", e)
                    }))
                    .map(|_| ())
                    .await
            }
        });

    pipe.forever();

    (poi_receiver, report_receiver)
}

pub async fn cross_check_poi<I>(
    poi1: ProofOfIndexing<I>,
    poi2: ProofOfIndexing<I>,
    mut poi_broadcaster: Sender<ProofOfIndexing<I>>,
) -> Result<POICrossCheckReport<I>, anyhow::Error>
where
    I: Indexer,
{
    info!(
        indexer1 = %poi1.indexer.id(),
        indexer2 = %poi2.indexer.id(),
        poi1 = %poi1.proof_of_indexing,
        poi2 = %poi2.proof_of_indexing,
        block = %poi1.block,
        deployment = %poi1.deployment.as_str(),
        "Cross-check POI"
    );

    // Broadcast these two POIs
    poi_broadcaster.send(poi1.clone()).await?;
    poi_broadcaster.send(poi2.clone()).await?;

    // If both POIs are identical, we're done
    if poi1.proof_of_indexing == poi2.proof_of_indexing {
        return Ok(POICrossCheckReport {
            poi1,
            poi2,
            diverging_block: None,
        });
    }

    // Bisect to find the first diverging/bad block

    let context = POIBisectContext {
        indexer1: poi1.indexer.clone(),
        indexer2: poi2.indexer.clone(),
        deployment: poi1.deployment.clone(),
        poi_broadcaster: poi_broadcaster.clone(),
    };

    let diverging_block = bisect_blocks(
        nanoid!(),
        context,
        DivergingBlock { poi1, poi2 },
        test_block_number,
    )
    .await?;

    info!(
        indexer1 = %diverging_block.poi1.indexer.id(),
        indexer2 = %diverging_block.poi2.indexer.id(),
        diverging_block = %diverging_block.poi1.block,
    );

    Ok(POICrossCheckReport {
        poi1: diverging_block.poi1,
        poi2: diverging_block.poi2,
        diverging_block: Some(()),
    })
}

async fn test_block_number<I>(
    bisection_id: String,
    ctx: POIBisectContext<I>,
    block_number: u64,
) -> Result<BisectDecision<I>, anyhow::Error>
where
    I: Indexer,
{
    debug!(
        %bisection_id,
        %block_number,
        "Comparing block",
    );

    let POIBisectContext {
        indexer1,
        indexer2,
        deployment,
        mut poi_broadcaster,
    } = ctx;

    let request = POIRequest {
        deployment: deployment.clone(),
        block_number,
    };

    let poi1 = indexer1.proof_of_indexing(request.clone()).await?;
    let poi2 = indexer2.proof_of_indexing(request.clone()).await?;

    poi_broadcaster.send(poi1.clone()).await?;
    poi_broadcaster.send(poi2.clone()).await?;

    debug!(
        %bisection_id,
        %block_number,
        poi1 = %poi1.proof_of_indexing,
        poi2 = %poi2.proof_of_indexing,
        "Comparing POIs at block"
    );

    if poi1.proof_of_indexing == poi2.proof_of_indexing {
        Ok(BisectDecision::Good) as Result<_, anyhow::Error>
    } else {
        Ok(BisectDecision::Bad { poi1, poi2 })
    }
}
