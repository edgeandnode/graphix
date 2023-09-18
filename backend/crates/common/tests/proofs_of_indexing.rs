use std::collections::BTreeSet;

use graphix_common::block_choice::BlockChoicePolicy;
use graphix_common::test_utils::fast_rng;
use graphix_common::test_utils::gen::gen_indexers;
use graphix_common::{metrics, queries};
use itertools::Itertools;

#[tokio::test]
async fn proofs_of_indexing() {
    // Run th test 100 times to increase likelyhood that randomness triggers a bug
    for i in 0..100 {
        let mut rng = fast_rng(i);
        let max_indexers = i;
        let indexers = gen_indexers(&mut rng, max_indexers as usize);

        let indexing_statuses = queries::query_indexing_statuses(indexers, metrics()).await;
        let pois =
            queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest);

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
