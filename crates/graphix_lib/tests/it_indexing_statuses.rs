use std::time::Duration;

use graphix_indexer_client::IndexerClient;
use graphix_lib::test_utils::{test_deployment_id, test_indexer_from_url};

#[tokio::test]
async fn send_indexer_statuses_query() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let test_deployment = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");

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
