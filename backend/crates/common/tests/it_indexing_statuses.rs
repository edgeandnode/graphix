use std::sync::Arc;
use std::time::Duration;

use graphix_common::config::{IndexerConfig, IndexerUrls};
use graphix_common::prelude::{Indexer, RealIndexer, SubgraphDeployment};
use reqwest::Url;

/// Test utility function to create a valid `Indexer` from an arbitrary base url.
fn test_indexer_from_url(url: impl Into<String>) -> Arc<impl Indexer> {
    let url: Url = url.into().parse().expect("Invalid status url");
    let conf = IndexerConfig {
        name: url.host().unwrap().to_string(),
        urls: IndexerUrls {
            status: url.join("status").unwrap(),
        },
    };
    Arc::new(RealIndexer::new(conf))
}

/// Test utility function to create a valid `SubgraphDeployment` with an arbitrary deployment
/// id/ipfs hash.
fn test_deployment_id(deployment: impl Into<String>) -> SubgraphDeployment {
    SubgraphDeployment(deployment.into())
}

#[tokio::test]
async fn send_indexer_statuses_query() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let test_deployment = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");

    //// When
    let request_fut = Indexer::indexing_statuses(indexer);
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
