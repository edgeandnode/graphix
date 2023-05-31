use graphix_common::{
    tests::{fast_rng, gen::gen_indexers},
    PrometheusMetrics,
};
use itertools::Itertools;
use std::collections::BTreeSet;

use crate::{indexing_statuses, proofs_of_indexing};

#[tokio::test]
async fn proofs_of_indexing() {
    // Run th test 100 times to increase likelyhood that randomness triggers a bug
    for i in 0..100 {
        let mut rng = fast_rng(i);
        let max_indexers = i;
        let indexers = gen_indexers(&mut rng, max_indexers as usize);

        let metrics =
            PrometheusMetrics::new(prometheus_exporter::prometheus::default_registry().clone());
        let indexing_statuses =
            indexing_statuses::query_indexing_statuses(&metrics, indexers).await;
        let pois = proofs_of_indexing::query_proofs_of_indexing(indexing_statuses);

        let actual_pois = pois.await.into_iter().collect::<BTreeSet<_>>();

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
