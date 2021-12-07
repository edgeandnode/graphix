use eventuals::{Eventual, EventualExt};
use futures::{channel::mpsc::channel, future, Stream};
use tracing::warn;

use crate::{
    indexer::Indexer,
    types::{Bytes32, SubgraphDeployment},
};

use super::{IndexerWithProofsOfIndexing, ProofOfIndexing};

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
    pois: Eventual<Vec<IndexerWithProofsOfIndexing>>,
) -> (
    impl Stream<Item = POISummary>,
    Eventual<Vec<POICrossCheckReport>>,
) {
    let (mut poi_sender, poi_receiver) = channel(1000);

    let reports = pois.subscribe().map(move |indexers| {
        // NOTE: This is just for testing POI writes.
        for indexer in indexers {
            for poi in indexer.proofs_of_indexing {
                if let Err(error) = poi_sender.try_send((&indexer.indexer.indexer, &poi).into()) {
                    warn!(%error, "Failed to forward POI")
                }
            }
        }

        // TODO: Fill this with real cross-checking logic.
        future::ready(vec![])
    });

    (poi_receiver, reports)
}
