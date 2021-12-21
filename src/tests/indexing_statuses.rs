use std::collections::BTreeSet;

use eventuals::Eventual;
use futures::{future, stream::FuturesUnordered, StreamExt};

use crate::{indexer::Indexer, indexing_statuses};

use super::gen::*;

#[tokio::test]
async fn indexing_statuses() {
    // Run the test 100 times to increase likelyhood that randomness triggers a bug
    for max_indexers in 0..100 {
        let indexers = gen_indexers(max_indexers);

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
            .collect::<BTreeSet<_>>();

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(indexers);

        let queried_statuses = indexing_statuses::indexing_statuses(indexers_reader)
            .subscribe()
            .next()
            .await
            .unwrap()
            .into_iter()
            .collect();

        assert_eq!(expected_statuses, queried_statuses);
    }
}
