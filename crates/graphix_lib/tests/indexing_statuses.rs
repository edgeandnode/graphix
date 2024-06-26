use futures::stream::FuturesUnordered;
use futures::{future, StreamExt};
use graphix_indexer_client::IndexingStatus;
use graphix_lib::indexing_loop::query_indexing_statuses;
use graphix_lib::metrics;
use graphix_lib::test_utils::fast_rng;
use graphix_lib::test_utils::gen::*;

#[tokio::test]
async fn indexing_statuses() {
    // Run the test 100 times to increase likelyhood that randomness triggers a bug
    for i in 0..100 {
        let mut rng = fast_rng(i);
        let max_indexers = i;

        let indexers = gen_indexers(&mut rng, max_indexers as usize);

        let expected_statuses = indexers
            .iter()
            .map(|indexer| indexer.clone().indexing_statuses())
            .collect::<FuturesUnordered<_>>()
            .filter_map(|result| match result {
                Ok(value) => future::ready(Some(value)),
                Err(_) => future::ready(None),
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let queried_statuses: Vec<IndexingStatus> = query_indexing_statuses(&indexers, metrics())
            .await
            .into_iter()
            .collect();

        assert_eq!(expected_statuses, queried_statuses);
    }
}
