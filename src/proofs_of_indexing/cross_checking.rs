use eventuals::{Eventual, EventualExt};
use futures::{channel::mpsc::channel, future, Stream};
use tracing::warn;

use crate::{
    indexer::Indexer,
    types::{Bytes32, SubgraphDeployment},
};

use super::ProofOfIndexing;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct POISummary {
    pub indexer: String,
    pub deployment: SubgraphDeployment,
    pub block_number: u64,
    pub block_hash: Bytes32,
    pub proof_of_indexing: Bytes32,
}

impl From<(&Indexer, &ProofOfIndexing)> for POISummary {
    fn from((indexer, poi): (&Indexer, &ProofOfIndexing)) -> Self {
        Self {
            indexer: indexer.id.clone(),
            deployment: poi.deployment.clone(),
            block_number: poi.block.number,
            block_hash: poi.block.hash.clone(),
            proof_of_indexing: poi.proof_of_indexing.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct POICrossCheckReport {}

pub fn cross_checking(
    pois: Eventual<Vec<ProofOfIndexing>>,
) -> (
    impl Stream<Item = POISummary>,
    Eventual<Vec<POICrossCheckReport>>,
) {
    let (mut poi_sender, poi_receiver) = channel(1000);

    let reports = pois.map(move |pois| {
        // Build a flat, unique list of all deployments we have POIs for
        let mut deployments = pois.iter().map(|poi| &poi.deployment).collect::<Vec<_>>();
        deployments.sort();
        deployments.dedup();

        // Build a map of deployments to Indexers/POIs
        let pois_by_deployment = deployments.iter().map(|deployment| {
            (
                deployment,
                pois.iter()
                    .filter(|poi| poi.deployment.eq(deployment))
                    .collect::<Vec<_>>(),
            )
        });

        // TODO: Add cross-checking logic here.

        future::ready(vec![])
    });

    (poi_receiver, reports)
}
