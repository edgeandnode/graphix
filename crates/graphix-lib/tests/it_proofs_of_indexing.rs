use std::sync::Arc;
use std::time::Duration;

use graphix_indexer_client::{Indexer, PoiRequest, RealIndexer, SubgraphDeployment};
use graphix_lib::config::{IndexerConfig, IndexerUrls};
use reqwest::Url;

/// Test utility function to create a valid `Indexer` from an arbitrary base url.
fn test_indexer_from_url(url: impl Into<String>) -> Arc<impl Indexer> {
    let url: Url = url.into().parse().expect("Invalid status url");
    let conf = IndexerConfig {
        name: Some(url.host().unwrap().to_string()),
        address: url.as_str().as_bytes().to_owned(),
        urls: IndexerUrls {
            status: url.join("status").unwrap(),
        },
    };
    Arc::new(RealIndexer::new(
        conf.name,
        conf.address,
        conf.urls.status.to_string(),
        todo!(),
    ))
}

/// Test utility function to create a valid `SubgraphDeployment` with an arbitrary deployment
/// id/ipfs hash.
fn test_deployment_id(deployment: impl Into<String>) -> SubgraphDeployment {
    SubgraphDeployment(deployment.into())
}

#[tokio::test]
async fn send_single_query_and_process_result() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let deployment = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");

    let poi_request = PoiRequest {
        deployment: deployment.clone(),
        block_number: 123,
    };

    //// When
    let request_fut = Indexer::proof_of_indexing(indexer, poi_request);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    //// Then
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(response.deployment, deployment);
    assert_eq!(response.block.number, 123);
}

#[tokio::test]
async fn send_single_query_of_unknown_deployment_id_and_handle_error() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let deployment_unknown = test_deployment_id("QmUnknownDeploymentId");

    let poi_request = PoiRequest {
        deployment: deployment_unknown.clone(),
        block_number: 123,
    };

    //// When
    let request_fut = Indexer::proof_of_indexing(indexer, poi_request);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    //// Then
    assert!(response.is_err());

    let response = response.unwrap_err();
    assert!(response
        .to_string()
        .contains("no proof of indexing returned"));
}

#[tokio::test]
async fn send_single_query_of_unknown_block_number_and_handle_error() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let deployment = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");

    let poi_request = PoiRequest {
        deployment: deployment.clone(),
        block_number: u64::MAX,
    };

    //// When
    let request_fut = Indexer::proof_of_indexing(indexer, poi_request);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    //// Then
    assert!(response.is_err());

    let response = response.unwrap_err();
    assert!(response
        .to_string()
        .contains("no proof of indexing returned"));
}

#[tokio::test]
async fn send_multiple_queries_and_process_results() {
    // Given

    // FIXME: This is temporarily set to 1 until we fix the error: 'Null value resolved for
    //  non-null field `proofOfIndexing`' Which is probably a Graph Node bug. Setting it to 1
    //  reduces the impact of this issue.
    const MAX_REQUESTS_PER_QUERY: usize = 1;

    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let deployment = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");

    let poi_requests = (1..=MAX_REQUESTS_PER_QUERY + 2)
        .map(|i| PoiRequest {
            deployment: deployment.clone(),
            block_number: i as u64,
        })
        .collect::<Vec<_>>();

    // When
    let request_fut = Indexer::proofs_of_indexing(indexer, poi_requests);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    // Then
    assert_eq!(response.len(), MAX_REQUESTS_PER_QUERY + 2);

    assert_eq!(response[0].deployment, deployment);
    assert_eq!(response[0].block.number, 1);

    assert_eq!(response[1].deployment, deployment);
    assert_eq!(response[1].block.number, 2);

    assert_eq!(response[2].deployment, deployment);
    assert_eq!(response[2].block.number, 3);
}

#[tokio::test]
async fn send_multiple_queries_of_unknown_deployment_id_and_process_results() {
    //// Given
    let indexer = test_indexer_from_url("https://testnet-indexer-03-europe-cent.thegraph.com");

    let deployment0 = test_deployment_id("QmeYTH2fK2wv96XvnCGH2eyKFE8kmRfo53zYVy5dKysZtH");
    let deployment1 = test_deployment_id("QmawxQJ5U1JvgosoFVDyAwutLWxrckqVmBTQxaMaKoj3Lw");
    let deployment_unknown = test_deployment_id("QmUnknownDeploymentId");

    let poi_requests = vec![
        PoiRequest {
            deployment: deployment0.clone(),
            block_number: 123,
        },
        PoiRequest {
            deployment: deployment_unknown.clone(),
            block_number: 42,
        },
        PoiRequest {
            deployment: deployment1.clone(),
            block_number: 456,
        },
    ];

    //// When
    let request_fut = Indexer::proofs_of_indexing(indexer, poi_requests);
    let response = tokio::time::timeout(Duration::from_secs(10), request_fut)
        .await
        .expect("Timeout");

    //// Then
    assert_eq!(response.len(), 2);

    assert_eq!(response[0].deployment, deployment0);
    assert_eq!(response[0].block.number, 123);

    assert_eq!(response[1].deployment, deployment1);
    assert_eq!(response[1].block.number, 456);
}
