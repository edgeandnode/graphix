use std::collections::BTreeSet;

use eventuals::{Eventual, EventualExt};
use futures::{
    channel::mpsc::{channel, Sender},
    stream::FuturesUnordered,
    FutureExt, SinkExt, Stream, StreamExt,
};
use itertools::Itertools;
use tracing::{info, warn};

use crate::{
    indexer::Indexer,
    types::{POICrossCheckReport, ProofOfIndexing},
};

pub fn cross_checking<T>(
    pois: Eventual<Vec<ProofOfIndexing<T>>>,
) -> (
    impl Stream<Item = ProofOfIndexing<T>>,
    Eventual<Vec<POICrossCheckReport<T>>>,
)
where
    T: Indexer + 'static,
{
    let (poi_broadcaster, poi_receiver) = channel(1000);

    let reports = pois.map(move |mut pois| {
        // Sort POIs (to make everything a little more predictable)
        pois.sort();

        // Build a flat, unique list of all deployments we have POIs for
        let deployments = pois
            .iter()
            .map(|poi| &poi.deployment)
            .collect::<BTreeSet<_>>();

        // Build a map of deployments to Indexers/POIs
        deployments
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
            .map(|(deployment, pois)| {
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
            .flatten()
            .map(|(poi1, poi2)| cross_check_poi(poi1, poi2, poi_broadcaster.clone()))
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .map(|reports| reports.into_iter().flatten().collect())
    });

    (poi_receiver, reports)
}

async fn cross_check_poi<T>(
    poi1: ProofOfIndexing<T>,
    poi2: ProofOfIndexing<T>,
    mut poi_broadcaster: Sender<ProofOfIndexing<T>>,
) -> Result<POICrossCheckReport<T>, anyhow::Error>
where
    T: Indexer,
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

    // TODO: Implement cross-checking for POIs that are different
    todo!()
}
