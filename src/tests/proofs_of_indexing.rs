use std::collections::BTreeSet;

use eventuals::Eventual;
use itertools::Itertools;

use crate::{indexing_statuses, proofs_of_indexing};

use super::gen_indexers;

#[tokio::test]
async fn proofs_of_indexing() {
    // Run th test 100 times to increase likelyhood that randomness triggers a bug
    for max_indexers in 0..100 {
        let indexers = gen_indexers(max_indexers);

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(indexers.clone());

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);

        let actual_pois = proofs_of_indexing_reader
            .subscribe()
            .next()
            .await
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>();

        // Assert that for every deployment, the POIs are for the same block
        // (across all indexers)
        assert!(actual_pois
            .iter()
            .group_by(|poi| poi.deployment.clone())
            .into_iter()
            .all(|(_, pois)| { pois.into_iter().map(|poi| &poi.block).all_equal() }));

        // NOTE: Add more assertions later.
    }
}

#[tokio::test]
async fn individual_poi_cross_checking() {
    for _ in 0..100 {
        let indexers = gen_indexers(2);

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(indexers.clone());

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);
    }
}
