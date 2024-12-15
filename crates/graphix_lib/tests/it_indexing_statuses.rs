use std::time::Duration;

use graphix_indexer_client::IndexerClient;
use graphix_lib::test_utils::{deployments, indexers, ipfs_cid, test_indexer_from_url};

#[tokio::test]
async fn send_indexer_statuses_query() {
    //// Given
    let indexer = test_indexer_from_url(indexers::ARB1_DATA_NEXUS);

    let test_deployment = ipfs_cid(deployments::ARB1_QUICKSWAP_V3);

    //// When
    let request_fut = IndexerClient::indexing_statuses(indexer);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    //// Then
    assert!(response.is_ok());

    let response = response.unwrap();
    assert!(!response.is_empty());
    assert!(response
        .iter()
        .any(|status| status.deployment == test_deployment));
}
